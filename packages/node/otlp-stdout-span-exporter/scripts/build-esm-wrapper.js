#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

// ESM wrapper content with webpack compatibility
const esmWrapper = `// ESM wrapper for CommonJS module
import { createRequire } from "module";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const require = createRequire(import.meta.url);

// Try to load the CommonJS module with multiple strategies
let mod;
try {
  // First try: Direct relative path (works in normal Node.js)
  mod = require("./index.js");
} catch (e1) {
  try {
    // Second try: Absolute path (might work in some bundlers)
    mod = require(join(__dirname, "index.js"));
  } catch (e2) {
    try {
      // Third try: Load via package name (for bundled environments)
      mod = require("@dev7a/otlp-stdout-span-exporter");
    } catch (e3) {
      // Last resort: If we're in a webpack bundle, the CommonJS module might be available globally
      if (typeof __webpack_require__ !== 'undefined') {
        throw new Error("ESM wrapper is not compatible with webpack bundling. Please use CommonJS require() instead of import.");
      }
      throw new Error(\`Failed to load CommonJS module. Tried:
1. ./index.js - \${e1.message}
2. \${join(__dirname, "index.js")} - \${e2.message}  
3. @dev7a/otlp-stdout-span-exporter - \${e3.message}\`);
    }
  }
}

// Re-export all named exports
export const OTLPStdoutSpanExporter = mod.OTLPStdoutSpanExporter;
export const LogLevel = mod.LogLevel;
export const OutputType = mod.OutputType;
export const VERSION = mod.VERSION;

// Default export
export default mod.OTLPStdoutSpanExporter;
`;

// Ensure dist directory exists
const distPath = path.join(__dirname, '..', 'dist');
if (!fs.existsSync(distPath)) {
  fs.mkdirSync(distPath, { recursive: true });
}

// Write the ESM wrapper
const outputPath = path.join(distPath, 'index.mjs');
fs.writeFileSync(outputPath, esmWrapper);

console.log('ESM wrapper created at:', outputPath); 