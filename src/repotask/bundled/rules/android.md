# Android Rules

- Preserve lifecycle ownership and cancellation behavior.
- Review coroutine and Flow collection for leaks, stale state, and incorrect dispatchers.
- Keep Android-specific logic out of generic modules.
- For Room changes, never modify an existing committed schema file. Bump the database version,
  add the new schema, and verify the matching migration.

