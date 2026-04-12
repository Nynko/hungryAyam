ALTER TABLE users
  ADD COLUMN email_verified_at timestamptz,
  ADD COLUMN email_verification_token text,
  ADD COLUMN email_verification_token_expires_at timestamptz;
