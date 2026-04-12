ALTER TABLE availability_rules
DROP CONSTRAINT public_holidays_consistent,
DROP COLUMN public_holidays_mode,
DROP COLUMN public_holidays_country;
