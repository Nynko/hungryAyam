ALTER TABLE users
  DROP COLUMN IF EXISTS email_verified_at,
  DROP COLUMN IF EXISTS email_verification_token,
  DROP COLUMN IF EXISTS email_verification_token_expires_at;
