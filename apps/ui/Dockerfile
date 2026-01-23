FROM archlinux:base AS node-base
RUN pacman -Sy --noconfirm
RUN pacman -S --noconfirm --needed ca-certificates curl gnupg nodejs npm icu

FROM node-base AS deps
WORKDIR /app
COPY apps/ui/package*.json ./
RUN npm ci

FROM node-base AS builder
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1

# Production defaults for build-time env vars
ARG NEXT_PUBLIC_MAIN_DOMAIN=reauth.dev
ENV NEXT_PUBLIC_MAIN_DOMAIN=${NEXT_PUBLIC_MAIN_DOMAIN}
COPY --from=deps /app/node_modules ./node_modules
COPY apps/ui/ .

# Remove local dev env file if it exists
RUN rm -f .env.local

RUN npm run build

FROM node-base AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

COPY --from=builder /app/.next ./.next
COPY --from=builder /app/package*.json ./
COPY --from=builder /app/node_modules ./node_modules

EXPOSE 3000
CMD ["npm", "run", "start"]
