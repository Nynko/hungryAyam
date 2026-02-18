# ── Stage 1: Build with Deno + Vite ───────────────────────────────
FROM denoland/deno:latest AS builder

WORKDIR /build

# App config build args — baked into the static bundle by Vite
ARG VITE_APP_TITLE=HungryAyam
ARG VITE_APP_IMAGE_URL=

ENV VITE_APP_TITLE=$VITE_APP_TITLE
ENV VITE_APP_IMAGE_URL=$VITE_APP_IMAGE_URL

# Copy dependency manifests first for caching
COPY package.json deno.json deno.lock* ./

# Install dependencies
RUN deno install --node-modules-dir

# Copy the rest of the source
COPY . .

# Build static files
RUN deno run --allow-all npm:vite build

# ── Stage 2: Serve with nginx ─────────────────────────────────────
FROM nginx:alpine

# Remove default nginx config
RUN rm /etc/nginx/conf.d/default.conf

# Copy our nginx config
COPY nginx.conf /etc/nginx/conf.d/default.conf

# Copy built static files
COPY --from=builder /build/dist /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]