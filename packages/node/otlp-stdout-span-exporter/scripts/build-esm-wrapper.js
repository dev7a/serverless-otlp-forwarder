#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

// ESM wrapper content
const esmWrapper = `// ESM wrapper for CommonJS module
import { createRequire } from "module";
const require = createRequire(import.meta.url);
const mod = require("./index.js");

// Re-export all named exports
export const OTLPStdoutSpanExporter = mod.OTLPStdoutSpanExporter;
export const LogLevel = mod.LogLevel;
export const OutputType = mod.OutputType;

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