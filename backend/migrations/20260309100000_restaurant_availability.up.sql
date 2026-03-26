-- Add availability rule support to restaurants
alter table restaurants
add column availability_rule_id uuid references availability_rules(id);