import { FullConfig } from '@playwright/test';

/**
 * Global setup function - runs once before all tests
 */
export default async function globalSetup(config: FullConfig) {
  const { baseURL } = config.projects[0].use;
  
  console.log('🚀 UltraDAG E2E Test Suite Starting...');
  console.log(`📍 Base URL: ${baseURL}`);
  console.log(`🌐 Projects: ${config.projects.map(p => p.name).join(', ')}`);
  
  // Store setup timestamp for test reporting
  const setupTime = new Date().toISOString();
  console.log(`⏰ Setup started at: ${setupTime}`);
  
  // You can perform global setup tasks here:
  // - Start mock servers
  // - Seed test databases
  // - Generate test data
  // - Setup authentication tokens
  
  // Example: Store base URL in a file for tests to use
  // import { writeFileSync } from 'fs';
  // writeFileSync('.test-env.json', JSON.stringify({ baseURL, setupTime }));
}
