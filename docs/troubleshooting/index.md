---
layout: default
title: Troubleshooting
nav_order: 7
has_children: true
---

# Troubleshooting

This guide helps you diagnose and resolve common issues with the Lambda OTLP Forwarder.

## Common Issues

### No Data in Collector
- Check CloudWatch Logs subscription filters
- Verify collector endpoint configuration
- Check authentication settings
- Validate OTLP data format

### Performance Issues
- Review memory configuration
- Check compression settings
- Monitor cold start impact
- Analyze batching configuration

### Configuration Problems
- Validate SAM template parameters
- Check environment variables
- Review IAM permissions
- Verify secrets configuration

## Debugging Tools

### CloudWatch Logs Insights
Query examples for troubleshooting:
```sql
fields @timestamp, @message
| filter @message like /ERROR/
| sort @timestamp desc
| limit 20
```

### CloudWatch Metrics
Key metrics to monitor:
- Invocation errors
- Processing duration
- Memory usage
- Throttling events

## Getting Help

1. Check the [FAQ](faq)
2. Review [Known Issues](known-issues)
3. Search [GitHub Issues](https://github.com/dev7a/lambda-otlp-forwarder/issues)
4. Create a new issue if needed

## Best Practices

- Enable detailed logging during troubleshooting
- Use test environments for validation
- Monitor resource usage
- Keep dependencies updated 