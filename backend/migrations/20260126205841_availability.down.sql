-- Add down migration script here
alter table items drop column if exists availability_rule_id;
alter table offers drop column if exists availability_rule_id;
alter table menus drop column if exists availability_rule_id;

drop table if exists availability_rules;
