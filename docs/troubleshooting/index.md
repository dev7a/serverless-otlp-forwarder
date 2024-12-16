---
layout: default
title: Troubleshooting
nav_order: 7
has_children: true
---

# Troubleshooting
{: .fs-9 }

Comprehensive guide to diagnose and resolve issues with Serverless OTLP Forwarder.
{: .fs-6 .fw-300 }

## Quick Diagnostics
{: .text-delta }

### Health Check
{: .text-delta }

```bash
# Check forwarder health
aws lambda invoke \
  --function-name otlp-forwarder-health \
  --payload '{"action": "health"}' \
  response.json

# Check subscription filter
aws logs describe-subscription-filters \
  --log-group-name /aws/lambda/your-function
```

### Common Status Codes
{: .text-delta }

| Code | Description | Action |
|:-----|:------------|:-------|
| `200` | Success | Normal operation |
| `400` | Invalid request | Check payload format |
| `401` | Unauthorized | Verify credentials |
| `403` | Forbidden | Check IAM permissions |
| `408` | Timeout | Adjust timeout settings |
| `429` | Throttling | Check concurrency limits |
| `500` | Internal error | Check CloudWatch logs |
| `503` | Service unavailable | Verify collector status |

## Common Issues
{: .text-delta }

### No Data in Collector
{: .text-delta }

{: .warning }
Checklist:
1. Verify subscription filters
2. Check collector endpoint
3. Validate authentication
4. Review IAM permissions
5. Check network connectivity

```bash
# Verify subscription filter
aws logs describe-subscription-filters \
  --log-group-name /aws/lambda/your-function

# Test collector connectivity
aws lambda invoke \
  --function-name otlp-forwarder \
  --payload '{"action": "test_connection"}' \
  response.json
```

### Performance Issues
{: .text-delta }

{: .warning }
Common causes:
1. Insufficient memory
2. Network latency
3. Collector bottleneck
4. High concurrency
5. Large batch sizes

```bash
# Check memory usage
aws cloudwatch get-metric-statistics \
  --namespace AWS/Lambda \
  --metric-name MemoryUsed \
  --dimensions Name=FunctionName,Value=otlp-forwarder \
  --start-time $(date -u -v-1H +%Y-%m-%dT%H:%M:%SZ) \
  --end-time $(date -u +%Y-%m-%dT%H:%M:%SZ) \
  --period 300 \
  --statistics Maximum

# Monitor concurrent executions
aws cloudwatch get-metric-statistics \
  --namespace AWS/Lambda \
  --metric-name ConcurrentExecutions \
  --dimensions Name=FunctionName,Value=otlp-forwarder \
  --start-time $(date -u -v-1H +%Y-%m-%dT%H:%M:%SZ) \
  --end-time $(date -u +%Y-%m-%dT%H:%M:%SZ) \
  --period 300 \
  --statistics Maximum
```

### Configuration Problems
{: .text-delta }

{: .warning }
Verify:
1. SAM template parameters
2. Environment variables
3. IAM roles and policies
4. Network configuration
5. Collector settings

## Debugging Tools
{: .text-delta }

### CloudWatch Logs Insights
{: .text-delta }

Query for errors:
```sql
fields @timestamp, @message
| filter @message like /ERROR/
| sort @timestamp desc
| limit 20
```

Query for performance:
```sql
fields @timestamp,
       @message,
       @logStream,
       @billedDuration,
       @maxMemoryUsed
| filter @type = "REPORT"
| sort @timestamp desc
| limit 20
```

Query for specific spans:
```sql
fields @timestamp,
       @message,
       spans.name,
       spans.duration
| filter ispresent(spans.name)
| sort duration desc
| limit 20
```

### CloudWatch Metrics
{: .text-delta }

Key metrics to monitor:

{: .info }
**Execution Metrics**
- `Invocations`
- `Errors`
- `Duration`
- `ConcurrentExecutions`
- `Throttles`

{: .info }
**Memory Metrics**
- `MemoryUsed`
- `MaxMemoryUsed`
- `MemorySize`
- `GCPauseDuration`

{: .info }
**Custom Metrics**
- `ProcessedLogEvents`
- `ForwardedSpans`
- `ProcessingErrors`
- `ForwardingLatency`

## Network Debugging
{: .text-delta }

```bash
# Test VPC connectivity
aws lambda invoke \
  --function-name otlp-forwarder \
  --payload '{
    "action": "test_network",
    "target": "collector.example.com",
    "port": 4318
  }' \
  response.json

# Check DNS resolution
aws lambda invoke \
  --function-name otlp-forwarder \
  --payload '{
    "action": "test_dns",
    "hostname": "collector.example.com"
  }' \
  response.json
```

## Error Reference
{: .text-delta }

### Common Error Codes
{: .text-delta }

| Error Code | Description | Solution |
|:-----------|:------------|:---------|
| `ECONNREFUSED` | Connection refused | Check network/firewall |
| `ETIMEDOUT` | Connection timeout | Increase timeout/check network |
| `ENOTFOUND` | DNS resolution failed | Check DNS configuration |
| `EPIPE` | Broken pipe | Check collector status |
| `ECONNRESET` | Connection reset | Check network stability |

### Error Messages
{: .text-delta }

{: .warning }
Common error messages and solutions:

1. **"Unable to forward spans"**
   - Check collector endpoint
   - Verify authentication
   - Check network connectivity

2. **"Memory limit exceeded"**
   - Increase memory allocation
   - Optimize batch size
   - Check for memory leaks

3. **"Subscription filter not found"**
   - Verify log group configuration
   - Check IAM permissions
   - Review CloudFormation stack

## Best Practices
{: .text-delta }

### Monitoring
{: .text-delta }

{: .info }
- Set up CloudWatch alarms
- Monitor key metrics
- Configure log retention
- Use X-Ray tracing
- Implement custom metrics

### Logging
{: .text-delta }

{: .info }
- Use structured logging
- Include correlation IDs
- Log appropriate detail level
- Configure log expiration
- Monitor log volume

### Testing
{: .text-delta }

{: .info }
- Use test events
- Monitor test results
- Validate configurations
- Check performance
- Verify security

## Getting Help
{: .text-delta }

1. Check the [FAQ](faq)
2. Review [Known Issues](known-issues)
3. Search [GitHub Issues](https://github.com/dev7a/serverless-otlp-forwarder/issues)
4. Join our [Discord Community](https://discord.gg/example)
5. Contact [Support](support)

## Next Steps
{: .text-delta }

- [Monitoring Setup](../deployment/monitoring) 