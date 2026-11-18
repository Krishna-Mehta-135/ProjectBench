FROM node:22-alpine
WORKDIR /web
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY . ./
RUN npm run build
EXPOSE 8080
CMD ["node", "serve.mjs"]
