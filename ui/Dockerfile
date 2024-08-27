FROM node:20-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .

# build
RUN npm run build

# serve
FROM nginx:alpine
COPY --from=0 /app/build /usr/share/nginx/html
COPY --from=0 /app/build/output.css /usr/share/nginx/html/output.css
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
