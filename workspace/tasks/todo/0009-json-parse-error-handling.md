Handle JSON parse errors in persistence
Stop swallowing JSON parse failures and surface or log them.

Checklist
- [ ] Find unwrap_or_default JSON parsing sites
- [ ] Add error logging/propagation
- [ ] Add tests or migration check if needed

History
- 2026-01-01 06:52 Created from code review finding #9 Silently swallowed JSON parse errors.
- 2026-01-01 06:55 Renamed file to 0009-json-parse-error-handling.md to use 4-digit task numbering.
