import { describe, expect, it } from '@jest/globals';
import * as fs from 'fs';
import * as path from 'path';

describe('Package exports', () => {
  // Get the package.json data
  const packageJsonPath = path.resolve(__dirname, '../package.json');
  const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));

  it('should have all required export paths defined', () => {
    expect(packageJson.exports).toBeDefined();
    expect(packageJson.exports['.'].types).toBe('./dist/index.d.ts');
    expect(packageJson.exports['.'].default).toBe('./dist/index.js');

    expect(packageJson.exports['./extension'].types).toBe('./dist/extension/index.d.ts');
    expect(packageJson.exports['./extension'].default).toBe('./dist/extension/index.js');

    expect(packageJson.exports['./telemetry'].types).toBe('./dist/telemetry/index.d.ts');
    expect(packageJson.exports['./telemetry'].default).toBe('./dist/telemetry/index.js');

    expect(packageJson.exports['./extractors'].types).toBe('./dist/internal/telemetry/extractors.d.ts');
    expect(packageJson.exports['./extractors'].default).toBe('./dist/internal/telemetry/extractors.js');
  });

  it('should have compiled extractors directory and files', () => {
    // Check if dist/extractors exists and contains the expected files
    const extractorsDir = path.resolve(__dirname, '../dist/extractors');
    expect(fs.existsSync(extractorsDir)).toBe(true);
    expect(fs.existsSync(path.join(extractorsDir, 'index.js'))).toBe(true);
    expect(fs.existsSync(path.join(extractorsDir, 'index.d.ts'))).toBe(true);
  });

  it('should expose all necessary extractors from the dedicated subpath', async () => {
    // When in the test environment, we can directly import from the dist files
    // to check that all expected exports are available
    // Use dynamic import to avoid linting issues
    const extractorsModule = await import('../dist/extractors/index');
    
    const expectedExports = [
      'apiGatewayV1Extractor',
      'apiGatewayV2Extractor',
      'albExtractor',
      'defaultExtractor',
      'TriggerType'
    ];
    
    for (const exportName of expectedExports) {
      expect(extractorsModule).toHaveProperty(exportName);
    }
  });
});
