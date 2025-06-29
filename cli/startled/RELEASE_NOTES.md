# Release Notes for startled v0.9.0

This release introduces significant updates to the `startled` CLI tool, focusing on the addition of a production-ready proxy Lambda function for enhanced benchmarking capabilities, improved documentation, and better security configurations. It also includes updates to versioning, new files for the proxy application, and enhancements to error handling in the proxy function's implementation.

### Proxy Lambda Function Enhancements

* **Proxy Function SAR Application Documentation**: Added detailed documentation for deploying the proxy function via AWS Serverless Application Repository (SAR) with support for AWS Console, AWS CLI, and SAM CLI. Security parameters such as `FunctionName`, `TargetFunctionResource`, and `PrincipalOrgID` are now documented. [[1]](diffhunk://#diff-bd0cb949bb67fcfa38060059b5016cdb217ed459714094210b87496f0714b453R8-R21) [[2]](diffhunk://#diff-aff33bf4e337463eb1a6180a9b58b944752069b10f5ca6a932555ac84afe0573R109-R179) [[3]](diffhunk://#diff-aff33bf4e337463eb1a6180a9b58b944752069b10f5ca6a932555ac84afe0573L428-R500) [[4]](diffhunk://#diff-ab5303c476f248d5b47eaf1c0c7cc172a9b41e67adeb3de47938e8e0250576aeR1-R110)
* **Proxy Function Implementation**: Improved error handling in the proxy Lambda function (`cli/startled/proxy/src/main.rs`) to provide user-friendly and detailed error messages for AWS service errors. Updated AWS SDK configuration to use the latest behavior version. [[1]](diffhunk://#diff-a9b3fbc2781ab6c4163776e1788312198e2db5b53a0de0ee43b71cb2e9ccb02aL68-R120) [[2]](diffhunk://#diff-a9b3fbc2781ab6c4163776e1788312198e2db5b53a0de0ee43b71cb2e9ccb02aL99-R141)

### New Proxy Application Files

* **Proxy Application Files**: Introduced new files for the proxy Lambda function, including `Cargo.toml`, `LICENSE`, `Makefile`, `VERSION`, and `samconfig.toml`. These files enable local development, deployment, and publishing to SAR. [[1]](diffhunk://#diff-21b2e198d1b8f15ae1e4274be9cdcfa2b74a9228cab2407ffb47322d58b24144R1-R21) [[2]](diffhunk://#diff-f54b1ffa95e9d46c6339dd286180c26b483fc6ea865c9a0c17371af6ed40b81aR1-R21) [[3]](diffhunk://#diff-98386d6377f696e6628afa8eb61d861080d80e358185ceeccb308b01d709e1caR1-R86) [[4]](diffhunk://#diff-126b799fd1cadd7d464993b09862a5a6b6da5b849929abcef5d87f47408f3cddR1) [[5]](diffhunk://#diff-34a114b574ecba0c4655c12c4f3a742f5493fab99154db1ef8a90a3b85b1df7eR1-R18)
* **`.gitignore` Updates**: Added entries for build artifacts and temporary files specific to the proxy application.

### Documentation Updates

* **Enhanced Prerequisites**: Updated the `README.md` to include AWS SAM CLI as an optional prerequisite for deploying the proxy function.
* **Better Installation Guidance**: Improved installation instructions by providing multiple deployment options for the proxy function. [[1]](diffhunk://#diff-aff33bf4e337463eb1a6180a9b58b944752069b10f5ca6a932555ac84afe0573R109-R179) [[2]](diffhunk://#diff-ab5303c476f248d5b47eaf1c0c7cc172a9b41e67adeb3de47938e8e0250576aeR1-R110)

### Versioning Updates

* **Version Bump**: Updated the version of the `startled` CLI tool to `0.9.0` in `Cargo.toml` and added a changelog entry for the new version. [[1]](diffhunk://#diff-112c3857fa8d5706869ef8ba5fdaee05097cf93d95f849ab39a4c1457fafa30bL3-R3) [[2]](diffhunk://#diff-bd0cb949bb67fcfa38060059b5016cdb217ed459714094210b87496f0714b453R8-R21)