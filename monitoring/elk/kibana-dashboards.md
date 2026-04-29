# Kibana saved objects provisioning
# Import via: curl -X POST kibana:5601/api/saved_objects/_import -H "kbn-xsrf: true" --form file=@kibana-dashboards.ndjson

# Dashboard: Application Logs Overview
# - Log level distribution (info/warn/error/critical)
# - Error rate over time
# - Top error messages
# - Request latency percentiles
# - Slow query log entries

# Alert: Error spike (>10 errors/min for 5 consecutive minutes)
# Alert: Critical log entry (immediate)
# Alert: No logs received for 10 minutes (Filebeat down)
