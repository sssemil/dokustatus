FROM node:20-alpine AS sdk-builder
WORKDIR /sdk
COPY libs/reauth-sdk-ts/package*.json ./
RUN npm install
COPY libs/reauth-sdk-ts/ .
RUN npm run build

FROM node:20-alpine AS deps
WORKDIR /app

# Copy demo API package files (without lock file)
COPY apps/demo_api/package.json ./

# Remove SDK from dependencies and install other deps
RUN sed -i '/"@reauth\/sdk"/d' package.json && \
    npm install

# Copy built SDK directly to node_modules
COPY --from=sdk-builder /sdk ./node_modules/@reauth/sdk

FROM node:20-alpine AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY apps/demo_api/ .

# Build TypeScript
RUN npm run build

FROM node:20-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production

COPY --from=builder /app/dist ./dist
COPY --from=builder /app/package*.json ./
COPY --from=deps /app/node_modules ./node_modules

EXPOSE 3003
CMD ["node", "dist/index.js"]
