output "bucket_name" { value = aws_s3_bucket.assets.bucket }

output "db_backups_bucket" { value = aws_s3_bucket.db_backups.bucket }

output "backup_iam_role_arn" { value = aws_iam_role.backup.arn }
