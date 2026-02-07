create table order_sessions (
    id uuid primary key default gen_random_uuid(),
    restaurant_id uuid not null references restaurants(id) on delete cascade,
    start_date timestamptz not null default now(),
    end_date timestamptz not null default date_trunc('day', now()) + interval '11 hours', -- Default to 11 am
    status smallint not null, -- Enum to be defined by backend
    created_at timestamptz not null default now(),
    created_by uuid not null references users(id),
    updated_at timestamptz not null default now(),
    updated_by uuid not null references users(id)
);

create index idx_order_sessions_restaurant on order_sessions(restaurant_id);
