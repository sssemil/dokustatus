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

# Copy demo API package files
COPY apps/demo_api/package*.json ./

# Update SDK path for Docker context and install
# Use npm@10 to avoid "extraneous" bug in npm@11
RUN npm install -g npm@10 && \
    sed -i 's|file:../../libs/reauth-sdk-ts|file:./libs/reauth-sdk-ts|' package.json && \
    npm install

FROM node:22-alpine AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/libs ./libs
COPY apps/demo_api/ .

# Build TypeScript
RUN npm run build

FROM node:22-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production

COPY --from=builder /app/dist ./dist
COPY --from=builder /app/package*.json ./
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/libs ./libs

EXPOSE 3003
CMD ["node", "dist/index.js"]
