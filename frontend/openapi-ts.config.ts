/**
 * OpenAPI TypeScript client generation.
 *
 * The current backend exposes OpenAPI JSON at runtime. Save that JSON to
 * frontend/api.json, then run npm run generate-api to create typed clients.
 */
import { defineConfig } from '@hey-api/openapi-ts'

export default defineConfig({
  input: './api.json',
  output: {
    path: './src/lib/api-generated',
  },
  services: {
    asClass: false,
    name: 'RmqttThingsService',
    include: 'responses|requests|all',
    operationId: true,
    response: 'body',
  },
  client: 'axios',
})
