use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_cloudformation::Client as CfnClient;
use aws_sdk_cloudwatchlogs::Client as CwlClient;
use aws_sdk_sts::Client as StsClient;

use crate::cli::CliArgs; // Import CliArgs

// --- AWS Setup Public Function ---

pub struct AwsSetupResult {
    pub cwl_client: CwlClient,
    pub account_id: String,
    pub region_str: String,
    #[allow(dead_code)]
    pub partition: String,
    pub resolved_arns: Vec<String>,
}

pub async fn setup_aws_resources(args: &CliArgs) -> Result<AwsSetupResult> {
    // --- 1. Load AWS Config ---
    let region_provider =
        RegionProviderChain::first_try(args.aws_region.clone().map(aws_config::Region::new))
            .or_default_provider()
            .or_else(aws_config::Region::new("us-east-1")); // Default fallback region

    let mut config_loader =
        aws_config::defaults(aws_config::BehaviorVersion::latest()).region(region_provider);

    if let Some(profile) = args.aws_profile.clone() {
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
    let resolved_log_group_names = discover_log_group_names(&cfn_client, &cwl_client, args).await?;

    // --- Add validation step ---
    tracing::debug!("Validating discovered log group names...");
    let validated_log_group_names =
        validate_log_groups(&cwl_client, resolved_log_group_names).await?;
    tracing::debug!(
        "Validation complete. Valid names: {:?}",
        validated_log_group_names
    );

    // --- Validate count of *validated* names ---
    let group_count = validated_log_group_names.len(); // Use validated count
    if group_count == 0 {
        let error_msg = if args.stack_name.is_some() {
            format!("Stack '{}' contained 0 discoverable and valid LogGroup resources (checked Lambda@Edge variants).", args.stack_name.as_deref().unwrap_or("N/A"))
        } else {
            format!(
                "Pattern '{}' matched 0 valid log groups (checked Lambda@Edge variants).",
                args.log_group_pattern.as_deref().unwrap_or("N/A")
            )
        };
        return Err(anyhow::anyhow!(error_msg));
    } else if group_count > 10 {
        let (method, value) = if let Some(stack) = args.stack_name.as_deref() {
            ("Stack", stack)
        } else {
            (
                "Pattern",
                args.log_group_pattern.as_deref().unwrap_or("N/A"),
            )
        };
        let error_msg = format!(
            "{} '{}' resulted in {} valid log groups (max 10 allowed for live tail). Found: {:?}",
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
    args: &CliArgs,
) -> Result<Vec<String>> {
    if let Some(stack_name) = args.stack_name.as_deref() {
        discover_log_groups_from_stack(cfn_client, stack_name).await
    } else if let Some(pattern) = args.log_group_pattern.as_deref() {
        discover_log_groups_by_pattern(cwl_client, pattern).await
    } else {
        Err(anyhow::anyhow!(
            "Internal error: No log group pattern or stack name provided."
        ))
    }
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

/// Discovers log groups matching a pattern.
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

/// Validates a list of potential log group names, checking for Lambda@Edge variants if necessary.
pub async fn validate_log_groups(
    cwl_client: &CwlClient,
    initial_names: Vec<String>,
) -> Result<Vec<String>> {
    let checks = initial_names.into_iter().map(|name| {
        let client = cwl_client.clone(); // Clone client for concurrent use
        async move {
            let describe_result = client
                .describe_log_groups()
                .log_group_name_prefix(&name) // Check if the original name exists
                .limit(1)
                .send()
                .await;

            match describe_result {
                Ok(output) => {
                    if output.log_groups.is_some_and(|lgs| lgs.iter().any(|lg| lg.log_group_name.as_deref() == Some(&name))) {
                        tracing::debug!(log_group = %name, "Validated existing log group");
                        Ok(Some(name)) // Original name found and matches
                    } else {
                        check_lambda_edge_variant(&client, name).await // Try edge variant
                    }
                }
                Err(e) => {
                    if let Some(err) = e.as_service_error() {
                        match err.meta().code() {
                            Some("ResourceNotFoundException") => {
                                check_lambda_edge_variant(&client, name).await // Try edge variant
                            }
                            _ => {
                                let context_msg = format!("Failed to describe log group '{}' due to SDK service error code: {:?}", name, err.meta().code());
                                Err(anyhow::Error::new(e).context(context_msg))
                            }
                        }
                    } else {
                        Err(anyhow::Error::new(e).context(format!("Failed to describe log group '{}'", name)))
                    }
                }
            }
        }
    });

    let results = futures::future::join_all(checks).await;

    let mut validated_names = Vec::new();
    for result in results {
        match result {
            Ok(Some(name)) => validated_names.push(name),
            Ok(None) => {}           // Logged within the check, skip
            Err(e) => return Err(e), // Propagate the first error
        }
    }

    Ok(validated_names)
}

/// Helper to check for the Lambda@Edge log group variant.
async fn check_lambda_edge_variant(
    client: &CwlClient,
    original_name: String,
) -> Result<Option<String>> {
    const LAMBDA_PREFIX: &str = "/aws/lambda/";
    if original_name.starts_with(LAMBDA_PREFIX) {
        let base_name = original_name.strip_prefix(LAMBDA_PREFIX).unwrap();
        let edge_name = format!("/aws/lambda/us-east-1.{}", base_name);
        tracing::debug!(original_log_group = %original_name, edge_variant = %edge_name, "Original log group not found/matched, checking Lambda@Edge variant");

        match client
            .describe_log_groups()
            .log_group_name_prefix(&edge_name)
            .limit(1)
            .send()
            .await
        {
            Ok(output) => {
                if output.log_groups.is_some_and(|lgs| {
                    lgs.iter()
                        .any(|lg| lg.log_group_name.as_deref() == Some(&edge_name))
                }) {
                    tracing::debug!(log_group = %edge_name, "Found and validated Lambda@Edge log group variant"); // Keep info for success
                    Ok(Some(edge_name))
                } else {
                    tracing::warn!(log_group = %original_name, edge_variant = %edge_name, "Original log group and Lambda@Edge variant not found/matched. Skipping.");
                    Ok(None)
                }
            }
            Err(e) => {
                if let Some(err) = e.as_service_error() {
                    match err.meta().code() {
                        Some("ResourceNotFoundException") => {
                            tracing::warn!(log_group = %original_name, edge_variant = %edge_name, "Original log group and Lambda@Edge variant not found. Skipping.");
                            Ok(None)
                        }
                        _ => {
                            let context_msg = format!("Failed to describe Lambda@Edge variant '{}' due to SDK service error code: {:?}", edge_name, err.meta().code());
                            Err(anyhow::Error::new(e).context(context_msg))
                        }
                    }
                } else {
                    Err(anyhow::Error::new(e).context(format!(
                        "Failed to describe Lambda@Edge variant '{}'",
                        edge_name
                    )))
                }
            }
        }
    } else {
        tracing::warn!(log_group = %original_name, "Log group not found and does not match Lambda pattern. Skipping.");
        Ok(None)
    }
}
