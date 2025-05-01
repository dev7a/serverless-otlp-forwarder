import { Resource } from '@opentelemetry/resources';
import { createLogger } from '../logger';
import { ENV_VARS, DEFAULTS, RESOURCE_ATTRIBUTES } from '../../constants';

const logger = createLogger('resource');

/**
 * Create a Resource instance with AWS Lambda attributes and OTEL environment variables.
 *
 * This function combines AWS Lambda environment attributes with any OTEL resource attributes
 * specified via environment variables (OTEL_RESOURCE_ATTRIBUTES and OTEL_SERVICE_NAME).
 *
 * Resource attributes for configuration values are only set when the corresponding
 * environment variables are explicitly set, following the pattern:
 * 1. Environment variables (recorded as resource attributes only when explicitly set)
 * 2. Constructor parameters (not recorded as resource attributes)
 * 3. Default values (not recorded as resource attributes)
 *
 * @returns Resource instance with AWS Lambda and OTEL environment attributes
 */
export function getLambdaResource(): Resource {
  // Start with Lambda attributes
  const attributes: Record<string, string | number> = {
    'cloud.provider': 'aws',
  };

  // Map environment variables to attribute names
  const envMappings: Record<string, string> = {
    AWS_REGION: 'cloud.region',
    AWS_LAMBDA_FUNCTION_NAME: 'faas.name',
    AWS_LAMBDA_FUNCTION_VERSION: 'faas.version',
    AWS_LAMBDA_LOG_STREAM_NAME: 'faas.instance',
    AWS_LAMBDA_FUNCTION_MEMORY_SIZE: 'faas.max_memory',
  };

  // Helper function to parse memory value
  const parseMemoryValue = (key: string, value: string | undefined, defaultValue: string) => {
    try {
      attributes[key] = parseInt(value || defaultValue, 10) * 1024 * 1024; // Convert MB to bytes
    } catch (error) {
      logger.warn('Failed to parse memory value:', error);
    }
  };

  // Add attributes only if they exist in environment
  for (const [envVar, attrName] of Object.entries(envMappings)) {
    const value = process.env[envVar];
    if (value) {
      if (attrName === 'faas.max_memory') {
        parseMemoryValue(attrName, value, '128');
      } else {
        attributes[attrName] = value;
      }
    }
  }

  // Add service name (guaranteed to have a value)
  const serviceName =
    process.env[ENV_VARS.SERVICE_NAME] ||
    process.env.AWS_LAMBDA_FUNCTION_NAME ||
    DEFAULTS.SERVICE_NAME;
  attributes['service.name'] = serviceName;

  // Helper function to parse numeric attribute, only setting it when environment variable is explicitly set
  const parseNumericAttribute = (key: string, envVar: string | undefined) => {
    if (envVar !== undefined) {
      try {
        const parsedValue = parseInt(envVar, 10);
        if (!isNaN(parsedValue)) {
          attributes[key] = parsedValue;
        } else {
          logger.warn(
            `Failed to parse numeric attribute ${key} from value: ${envVar}, attribute not set`
          );
        }
      } catch (error) {
        logger.warn(
          `Error parsing numeric attribute ${key} from value: ${envVar}, attribute not set`,
          error
        );
      }
    }
  };

  // Add span processor mode attribute only when environment variable is set
  const processorModeEnv = process.env[ENV_VARS.PROCESSOR_MODE];
  if (processorModeEnv !== undefined) {
    attributes[RESOURCE_ATTRIBUTES.PROCESSOR_MODE] = processorModeEnv;
  }

  // Add queue size attribute only when environment variable is set
  parseNumericAttribute(RESOURCE_ATTRIBUTES.QUEUE_SIZE, process.env[ENV_VARS.QUEUE_SIZE]);

  // Add compression level attribute only when environment variable is set
  parseNumericAttribute(
    RESOURCE_ATTRIBUTES.COMPRESSION_LEVEL,
    process.env[ENV_VARS.COMPRESSION_LEVEL]
  );

  // Add OTEL environment resource attributes if present
  const envResourcesItems = process.env[ENV_VARS.RESOURCE_ATTRIBUTES];
  if (envResourcesItems) {
    for (const item of envResourcesItems.split(',')) {
      try {
        const [key, value] = item.split('=', 2);
        if (value?.trim()) {
          const valueUrlDecoded = decodeURIComponent(value.trim());
          attributes[key.trim()] = valueUrlDecoded;
        }
      } catch {
        // Skip malformed items
        continue;
      }
    }
  }

  // Create resource and merge with default resource
  const resource = new Resource(attributes);
  return Resource.default().merge(resource);
}
