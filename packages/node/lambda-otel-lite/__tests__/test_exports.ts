import { describe, expect, it } from '@jest/globals';
import * as fs from 'fs';
import * as path from 'path';
// No need to import types we're not using

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

  it('should have extractors directory in source', () => {
    // Check if src/extractors exists and contains the expected files
    const extractorsDir = path.resolve(__dirname, '../src/extractors');
    expect(fs.existsSync(extractorsDir)).toBe(true);
    expect(fs.existsSync(path.join(extractorsDir, 'index.ts'))).toBe(true);
  });

  it('should expose all necessary extractors from the source files', async () => {
    // Import directly from source files which are available during tests
    const extractorsModule = await import('../src/extractors/index');
    
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
