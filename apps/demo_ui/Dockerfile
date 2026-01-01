FROM node:22-alpine AS sdk-builder
WORKDIR /sdk
COPY libs/reauth-sdk-ts/package*.json ./
RUN npm install
COPY libs/reauth-sdk-ts/ .
RUN npm run build

FROM node:22-alpine AS deps
WORKDIR /app

# Copy built SDK
COPY --from=sdk-builder /sdk ./libs/reauth-sdk-ts

# Copy demo UI package files
COPY apps/demo_ui/package*.json ./

# Update SDK path for Docker context and install
RUN sed -i 's|file:../../libs/reauth-sdk-ts|file:./libs/reauth-sdk-ts|' package.json && npm install

FROM node:22-alpine AS builder
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1

# Production defaults for build-time env vars
ARG NEXT_PUBLIC_DOMAIN=anypost.xyz
ENV NEXT_PUBLIC_DOMAIN=${NEXT_PUBLIC_DOMAIN}
ARG NEXT_PUBLIC_UMAMI_WEBSITE_ID=
ENV NEXT_PUBLIC_UMAMI_WEBSITE_ID=${NEXT_PUBLIC_UMAMI_WEBSITE_ID}

COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/libs ./libs
COPY apps/demo_ui/ .

# Remove local dev env file if it exists
RUN rm -f .env.local

# Build Next.js
RUN npm run build

FROM node:22-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

COPY --from=builder /app/.next ./.next
COPY --from=builder /app/package*.json ./
COPY --from=builder /app/node_modules ./node_modules
COPY --from=deps /app/libs ./libs

EXPOSE 3002
CMD ["npm", "run", "start"]
