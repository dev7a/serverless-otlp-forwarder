//! Handles AWS SDK setup, client creation, and discovery of CloudWatch Log Groups.
//!
//! This module is responsible for:
//! 1. Initializing AWS configuration (region, credentials).
//! 2. Creating AWS service clients (CloudWatch Logs, CloudFormation, STS).
//! 3. Discovering relevant log group names based on user-provided patterns or
//!    CloudFormation stack names.
//! 4. Validating the existence of these log groups, including handling common
//!    Lambda@Edge naming conventions.
//! 5. Constructing ARNs for the validated log groups.

use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_cloudformation::Client as CfnClient;
use aws_sdk_cloudwatchlogs::Client as CwlClient;
use aws_sdk_sts::Client as StsClient;

// --- AWS Setup Public Function ---

pub struct AwsSetupResult {
    pub cwl_client: CwlClient,
    pub account_id: String,
    pub region_str: String,
    #[allow(dead_code)]
    pub partition: String,
    pub resolved_arns: Vec<String>,
}

pub async fn setup_aws_resources(
    log_group_pattern: &Option<Vec<String>>,
    stack_name: &Option<String>,
    aws_region: &Option<String>,
    aws_profile: &Option<String>,
) -> Result<AwsSetupResult> {
    // --- 1. Load AWS Config ---
    let region_provider =
        RegionProviderChain::first_try(aws_region.clone().map(aws_config::Region::new))
            .or_default_provider()
            .or_else(aws_config::Region::new("us-east-1")); // Default fallback region

    let mut config_loader =
        aws_config::defaults(aws_config::BehaviorVersion::latest()).region(region_provider);

    if let Some(profile) = aws_profile.clone() {
        config_loader = config_loader.profile_name(profile);
    }

    let aws_config = config_loader.load().await;
    tracing::debug!(
        "Logged in AWS config with region: {:?}",
        aws_config.region()
    );

    // --- 2. Create AWS Clients ---
    let cwl_client = CwlClient::new(&aws_config);
    tracing::debug!("CloudWatch Logs client created.");
    let cfn_client = CfnClient::new(&aws_config);
    tracing::debug!("CloudFormation client created.");
    let sts_client = StsClient::new(&aws_config);
    tracing::debug!("STS client created.");

    // --- Get Account ID and Region for ARN construction ---
    let region_str = aws_config
        .region()
        .ok_or_else(|| anyhow::anyhow!("Could not determine AWS region from config"))?
        .to_string();
    let caller_identity = sts_client
        .get_caller_identity()
        .send()
        .await
        .context("Failed to get caller identity from STS")?;
    let account_id = caller_identity
        .account()
        .ok_or_else(|| {
            anyhow::anyhow!("Could not determine AWS Account ID from STS caller identity")
        })?
        .to_string();
    let partition = "aws"; // Assuming standard AWS partition
    tracing::debug!(region = %region_str, account_id = %account_id, partition = %partition, "Determined region, account ID, and partition");

    // --- 5. Discover Log Groups based on pattern or stack name ---
    let resolved_log_group_names =
        discover_log_group_names(&cfn_client, &cwl_client, log_group_pattern, stack_name).await?;

    // --- Add validation step ---
    tracing::debug!("Validating discovered log group names...");
    let validated_log_group_names =
        validate_log_groups(&cwl_client, resolved_log_group_names, &region_str).await?;
    tracing::debug!(
        "Validation complete. Valid names: {:?}",
        validated_log_group_names
    );

    // --- Validate count of *validated* names ---
    let group_count = validated_log_group_names.len(); // Use validated count
    if group_count == 0 {
        let error_msg = if stack_name.is_some() {
            format!("Stack '{}' contained 0 discoverable and valid LogGroup resources (checked Lambda@Edge variants).", stack_name.as_deref().unwrap_or("N/A"))
        } else {
            format!(
                "Log Groups Patterns {:?} matched 0 valid log groups (checked Lambda@Edge variants).",
                log_group_pattern.as_ref().map_or(vec!["N/A".to_string()], |v| v.to_vec())
            )
        };
        return Err(anyhow::anyhow!(error_msg));
    } else if group_count > 10 {
        let (method, value) = if let Some(stack) = stack_name.as_deref() {
            ("Stack", stack.to_string())
        } else {
            (
                "Log Groups Patterns",
                format!(
                    "{:?}",
                    log_group_pattern
                        .as_ref()
                        .map_or(vec!["N/A".to_string()], |v| v.to_vec())
                ),
            )
        };
        let error_msg = format!(
            "{} {} resulted in {} valid log groups (max 10 allowed for live tail). Found: {:?}",
            method, value, group_count, validated_log_group_names
        );
        return Err(anyhow::anyhow!(error_msg));
    } else {
        tracing::debug!(
            "Proceeding with {} validated log group name(s): {:?}",
            group_count,
            validated_log_group_names
        );
    }

    // --- Construct ARNs from *validated* names ---
    let resolved_log_group_arns: Vec<String> = validated_log_group_names
        .iter()
        .map(|name| {
            format!(
                "arn:{}:logs:{}:{}:log-group:{}",
                partition, region_str, account_id, name
            )
        })
        .collect();
    tracing::debug!("Constructed ARNs: {:?}", resolved_log_group_arns);

    Ok(AwsSetupResult {
        cwl_client, // Return the CWL client for starting the tail
        account_id,
        region_str,
        partition: partition.to_string(), // Convert &str to String
        resolved_arns: resolved_log_group_arns,
    })
}

// --- Private Helper Functions ---

/// Discovers log group names based on stack or pattern arguments.
async fn discover_log_group_names(
    cfn_client: &CfnClient,
    cwl_client: &CwlClient,
    log_group_pattern: &Option<Vec<String>>,
    stack_name: &Option<String>,
) -> Result<Vec<String>> {
    // Create a HashSet to collect all log groups and avoid duplicates
    let mut all_log_groups = std::collections::HashSet::new();

    // Process stack name if provided
    if let Some(stack) = stack_name.as_deref() {
        let stack_groups = discover_log_groups_from_stack(cfn_client, stack).await?;
        for group in stack_groups {
            all_log_groups.insert(group);
        }
    }

    // Process log group patterns if provided
    if let Some(patterns) = log_group_pattern {
        if !patterns.is_empty() {
            let pattern_groups = discover_log_groups_by_patterns(cwl_client, patterns).await?;
            for group in pattern_groups {
                all_log_groups.insert(group);
            }
        }
    }

    // Return error if neither was provided or both were empty
    if all_log_groups.is_empty() {
        if stack_name.is_none() && log_group_pattern.is_none() {
            return Err(anyhow::anyhow!(
                "Internal error: No log group pattern or stack name provided."
            ));
        } else {
            return Err(anyhow::anyhow!(
                "No log groups found with the provided pattern(s) and/or stack name."
            ));
        }
    }

    // Convert to Vec and return
    Ok(all_log_groups.into_iter().collect())
}

/// Discovers log groups matching multiple patterns.
async fn discover_log_groups_by_patterns(
    cwl_client: &CwlClient,
    patterns: &[String],
) -> Result<Vec<String>> {
    tracing::debug!("Discovering log groups matching patterns: {:?}", patterns);

    // Use a HashSet to avoid duplicates when multiple patterns match the same log group
    let mut discovered_groups = std::collections::HashSet::new();

    // Process each pattern in sequence
    for pattern in patterns {
        // Call the existing function that handles a single pattern
        let groups = discover_log_groups_by_pattern(cwl_client, pattern).await?;
        // Add results to our set
        for group in groups {
            discovered_groups.insert(group);
        }
    }

    // Convert back to Vec for the return value
    Ok(discovered_groups.into_iter().collect())
}

/// Discovers log groups matching a single pattern.
async fn discover_log_groups_by_pattern(
    cwl_client: &CwlClient,
    pattern: &str,
) -> Result<Vec<String>> {
    tracing::debug!("Discovering log groups matching pattern: '{}'", pattern);
    let describe_output = cwl_client
        .describe_log_groups()
        .log_group_name_pattern(pattern)
        .send()
        .await
        .context("Failed to describe log groups")?;

    Ok(describe_output
        .log_groups
        .unwrap_or_default()
        .into_iter()
        .filter_map(|lg| lg.log_group_name)
        .collect())
}

/// Discovers log groups within a CloudFormation stack.
async fn discover_log_groups_from_stack(
    cfn_client: &CfnClient,
    stack_name: &str,
) -> Result<Vec<String>> {
    tracing::debug!("Discovering log groups from stack: '{}'", stack_name);
    let mut discovered_groups = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = cfn_client.list_stack_resources().stack_name(stack_name);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let output = request
            .send()
            .await
            .with_context(|| format!("Failed to list resources for stack '{}'", stack_name))?;

        if let Some(summaries) = output.stack_resource_summaries {
            for summary in summaries {
                if summary.resource_type.as_deref() == Some("AWS::Logs::LogGroup") {
                    if let Some(physical_id) = summary.physical_resource_id {
                        discovered_groups.push(physical_id);
                    } else {
                        tracing::warn!(resource_summary = ?summary, "Found LogGroup resource without physical ID");
                    }
                } else if summary.resource_type.as_deref() == Some("AWS::Lambda::Function") {
                    if let Some(physical_id) = summary.physical_resource_id {
                        let lambda_log_group_name = format!("/aws/lambda/{}", physical_id);
                        tracing::debug!(lambda_function = %physical_id, derived_log_group = %lambda_log_group_name, "Adding derived log group for Lambda function");
                        discovered_groups.push(lambda_log_group_name);
                    } else {
                        tracing::warn!(resource_summary = ?summary, "Found Lambda function resource without physical ID");
                    }
                }
            }
        }

        if let Some(token) = output.next_token {
            next_token = Some(token);
        } else {
            break;
        }
    }
    Ok(discovered_groups)
}

/// Validates a list of potential log group names, prioritizing Lambda@Edge patterns.
pub async fn validate_log_groups(
    cwl_client: &CwlClient,
    initial_names: Vec<String>,
    region_str: &str,
) -> Result<Vec<String>> {
    const LAMBDA_PREFIX: &str = "/aws/lambda/";

    let checks = initial_names.into_iter().map(|name| {
        let client = cwl_client.clone();
        let region = region_str.to_string();
        async move {
            if name.starts_with(LAMBDA_PREFIX) {
                let base_name_part = name.strip_prefix(LAMBDA_PREFIX).unwrap_or(&name);

                let potential_edge_name = format!("{}{}.{}", LAMBDA_PREFIX, region, base_name_part);
                tracing::debug!(log_group=%name, potential_edge_name=%potential_edge_name, "Checking for Lambda@Edge variant first");

                match describe_exact_log_group(&client, &potential_edge_name).await {
                    Ok(Some(found_edge_name)) => {
                        tracing::debug!(log_group=%found_edge_name, "Validated exact Lambda@Edge log group name");
                        Ok(Some(found_edge_name))
                    }
                    Ok(None) => {
                        tracing::debug!(log_group=%name, potential_edge_name=%potential_edge_name, "Lambda@Edge variant not found, checking original name");
                         match describe_exact_log_group(&client, &name).await {
                            Ok(Some(found_original_name)) => {
                                tracing::debug!(log_group=%found_original_name, "Validated original Lambda log group name after checking edge variant");
                                Ok(Some(found_original_name))
                            }
                            Ok(None) => {
                                tracing::warn!(log_group=%name, potential_edge_name=%potential_edge_name, "Neither Lambda@Edge variant nor original name found. Skipping.");
                                Ok(None)
                            }
                            Err(e) => Err(e.context(format!("Error validating original Lambda log group name '{}' after checking edge variant", name))),
                         }
                    }
                     Err(e) => Err(e.context(format!("Error validating potential Lambda@Edge log group name '{}'", potential_edge_name))),
                }
            } else {
                tracing::debug!(log_group=%name, "Checking non-Lambda log group name directly");
                 match describe_exact_log_group(&client, &name).await {
                     Ok(Some(found_name)) => {
                         tracing::debug!(log_group = %found_name, "Validated non-Lambda log group");
                          Ok(Some(found_name))
                     }
                     Ok(None) => {
                          tracing::warn!(log_group=%name, "Non-Lambda log group name not found. Skipping.");
                          Ok(None)
                     }
                     Err(e) => Err(e.context(format!("Error validating non-Lambda log group '{}'", name))),
                }
            }
        }
    });

    let results = futures::future::join_all(checks).await;

    let mut validated_names = Vec::new();
    let mut errors = Vec::new(); // Collect errors to potentially report them all

    for result in results {
        match result {
            Ok(Some(name)) => validated_names.push(name),
            Ok(None) => {}            // Logged within the check, skip
            Err(e) => errors.push(e), // Collect error
        }
    }

    // If any errors occurred during validation, return the first one
    if let Some(first_error) = errors.into_iter().next() {
        return Err(first_error);
    }

    Ok(validated_names)
}

/// Helper to describe a single log group by exact name.
async fn describe_exact_log_group(client: &CwlClient, name: &str) -> Result<Option<String>> {
    match client
        .describe_log_groups()
        .log_group_name_prefix(name) // Use prefix for API
        .limit(1)
        .send()
        .await
    {
        Ok(output) => {
            // Check if the *exact* name was returned
            if output.log_groups.is_some_and(|lgs| {
                lgs.iter()
                    .any(|lg| lg.log_group_name.as_deref() == Some(name))
            }) {
                Ok(Some(name.to_string()))
            } else {
                Ok(None) // Prefix matched something else, or nothing
            }
        }
        Err(e) => {
            // Specifically handle ResourceNotFoundException as Ok(None)
            if let Some(service_error) = e.as_service_error() {
                // Compare the error code string directly
                if service_error.meta().code() == Some("ResourceNotFoundException") {
                    return Ok(None);
                }
            }
            // Otherwise, it's an actual error
            Err(anyhow::Error::new(e).context(format!("Failed to describe log group '{}'", name)))
        }
    }
}
