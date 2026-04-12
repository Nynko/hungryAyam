-- Add public holiday constraint to availability rules
ALTER TABLE availability_rules
ADD COLUMN public_holidays_country text,        -- ISO 3166-1 alpha-2, e.g. 'FR'
ADD COLUMN public_holidays_mode text            -- 'exclude' | 'only'
    CHECK (public_holidays_mode IS NULL OR public_holidays_mode IN ('exclude', 'only')),
ADD CONSTRAINT public_holidays_consistent
    CHECK (
        (public_holidays_country IS NULL) = (public_holidays_mode IS NULL)
    );
