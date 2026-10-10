import { defineConfig } from '@playwright/test';
export default defineConfig({testDir:'./tests/browser',timeout:45000,workers:2,use:{baseURL:'http://localhost:49331',browserName:'chromium'},webServer:{command:'node node_modules/astro/astro.js preview --host 127.0.0.1 --port 49331',url:'http://localhost:49331',reuseExistingServer:!process.env.CI,timeout:30000}});
