Redact secrets from DB connection errors
Avoid leaking passwords in DATABASE_URL error output.

Checklist
- [ ] Review DB connection error mapping
- [ ] Redact or replace raw error details
- [ ] Add test or check error output format

History
- 2026-01-01 06:52 Created from code review finding #20 Password/secret not redacted in Database URL error.
- 2026-01-01 06:55 Renamed file to 0020-redact-db-url-error.md to use 4-digit task numbering.
