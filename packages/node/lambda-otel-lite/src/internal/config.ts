import { createLogger } from './logger';

const logger = createLogger('config');

/**
 * Helper functions for reading and validating environment variables with proper precedence.
 *
 * These functions implement the precedence logic:
 * 1. Environment variable (if set and valid)
 * 2. Programmatic configuration (if provided)
 * 3. Default value
 */

/**
 * Get a boolean value from an environment variable with proper precedence.
 *
 * Only "true" and "false" (case-insensitive) are accepted as valid values.
 * If the environment variable is set but invalid, a warning is logged and
 * the fallback value is used.
 *
 * @param name - Name of the environment variable
 * @param configValue - Optional programmatic configuration value
 * @param defaultValue - Default value if neither env var nor config is set
 * @returns The resolved boolean value
 */
export function getBooleanValue(
  name: string,
  configValue?: boolean,
  defaultValue: boolean = false
): boolean {
  const envValue = process.env[name];

  // If environment variable is set and not empty
  if (envValue !== undefined) {
    const value = envValue.trim().toLowerCase();
    if (value === 'true') {
      return true;
    }
    if (value === 'false') {
      return false;
    }

    // Invalid environment variable value - log warning and continue
    if (value) {
      logger.warn(
        `Invalid boolean for ${name}: ${envValue}. Must be 'true' or 'false'. Using fallback.`
      );
    }
  }

  // Use config value if provided, otherwise default
  return configValue !== undefined ? configValue : defaultValue;
}

/**
 * Get a numeric value from an environment variable with proper precedence.
 *
 * If the environment variable is set but invalid, a warning is logged and
 * the fallback value is used.
 *
 * @param name - Name of the environment variable
 * @param configValue - Optional programmatic configuration value
 * @param defaultValue - Default value if neither env var nor config is set
 * @param validator - Optional function to validate the parsed number
 * @returns The resolved numeric value
 */
export function getNumericValue(
  name: string,
  configValue?: number,
  defaultValue: number = 0,
  validator?: (value: number) => boolean
): number {
  const envValue = process.env[name];

  // If environment variable is set and not empty
  if (envValue !== undefined) {
    try {
      const parsedValue = parseInt(envValue.trim(), 10);
      if (!isNaN(parsedValue)) {
        // If a validator is provided, check the value
        if (validator && !validator(parsedValue)) {
          logger.warn(`Invalid value for ${name}: ${envValue}. Failed validation. Using fallback.`);
        } else {
          return parsedValue;
        }
      } else {
        logger.warn(`Invalid numeric value for ${name}: ${envValue}. Using fallback.`);
      }
    } catch {
      logger.warn(`Error parsing numeric value for ${name}: ${envValue}. Using fallback.`);
    }
  }

  // Use config value if provided, otherwise default
  return configValue !== undefined ? configValue : defaultValue;
}

/**
 * Get a string value from an environment variable with proper precedence.
 *
 * @param name - Name of the environment variable
 * @param configValue - Optional programmatic configuration value
 * @param defaultValue - Default value if neither env var nor config is set
 * @param validator - Optional function to validate the string
 * @returns The resolved string value
 */
export function getStringValue(
  name: string,
  configValue?: string,
  defaultValue: string = '',
  validator?: (value: string) => boolean
): string {
  const envValue = process.env[name];

  // If environment variable is set and not empty
  if (envValue !== undefined && envValue.trim() !== '') {
    const value = envValue.trim();

    // If a validator is provided, check the value
    if (validator && !validator(value)) {
      logger.warn(`Invalid value for ${name}: ${value}. Failed validation. Using fallback.`);
    } else {
      return value;
    }
  }

  // Use config value if provided, otherwise default
  return configValue !== undefined ? configValue : defaultValue;
}
