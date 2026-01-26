FROM node:20-alpine AS sdk-builder
WORKDIR /sdk
COPY libs/reauth-sdk-ts/package*.json ./
RUN npm install
COPY libs/reauth-sdk-ts/ .
RUN npm run build

FROM node:20-alpine AS deps
WORKDIR /app

# Copy demo UI package files
COPY apps/demo_ui/package*.json ./

# Remove SDK from dependencies, delete lockfile, and install other deps
RUN sed -i '/"@reauth\/sdk"/d' package.json && \
    rm -f package-lock.json && \
    npm install

# Copy built SDK directly to node_modules
COPY --from=sdk-builder /sdk ./node_modules/@reauth/sdk

FROM node:20-alpine AS builder
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1

# Production defaults for build-time env vars
ARG NEXT_PUBLIC_DOMAIN=anypost.xyz
ENV NEXT_PUBLIC_DOMAIN=${NEXT_PUBLIC_DOMAIN}
COPY --from=deps /app/node_modules ./node_modules
COPY apps/demo_ui/ .

# Remove local dev env file if it exists
RUN rm -f .env.local

# Build Next.js
RUN npm run build

FROM node:20-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

COPY --from=builder /app/.next ./.next
COPY --from=builder /app/package*.json ./
COPY --from=builder /app/node_modules ./node_modules

EXPOSE 3002
CMD ["npm", "run", "start"]
