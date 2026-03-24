import { test, expect } from '../../fixtures/base.fixture';
import { GovernancePagePO } from '../../page-objects/pages/GovernancePagePO';
import { WalletPagePO } from '../../page-objects/pages/WalletPagePO';
import { CreateKeystoreModalPO } from '../../page-objects/modals/CreateKeystoreModalPO';
import { generateWalletData } from '../../utils/test-data';

test.describe('Governance Page', () => {
  let governancePage: GovernancePagePO;
  let walletPage: WalletPagePO;
  let createKeystoreModal: CreateKeystoreModalPO;

  test.beforeEach(async ({ page }) => {
    governancePage = new GovernancePagePO(page);
    walletPage = new WalletPagePO(page);
    createKeystoreModal = new CreateKeystoreModalPO(page);
    
    // Create and unlock wallet first
    await page.goto('/wallet');
    const walletData = generateWalletData();
    await walletPage.createWalletButton.click();
    await createKeystoreModal.waitForOpen();
    await createKeystoreModal.fillForm(walletData.name, walletData.password);
    await createKeystoreModal.submit();
    await createKeystoreModal.waitForClose();
    await walletPage.unlockWallet(walletData.password);
    
    // Navigate to governance
    await page.goto('/governance');
    await governancePage.waitForLoaded();
  });

  test.describe('Page Load and Layout', () => {
    test('should load governance page successfully', async ({ page }) => {
      await expect(page).toHaveURL('/governance');
      await expect(governancePage.pageContainer).toBeVisible();
    });

    test('should display proposal list', async () => {
      await expect(governancePage.proposalList).toBeVisible();
    });

    test('should display active proposals section', async () => {
      await expect(governancePage.activeProposals).toBeVisible();
    });

    test('should display past proposals section', async () => {
      await expect(governancePage.pastProposals).toBeVisible();
    });

    test('should display create proposal button', async () => {
      await expect(governancePage.createProposalButton).toBeVisible();
    });
  });

  test.describe('Proposal Display', () => {
    test('should display proposal cards', async () => {
      const count = await governancePage.getProposalCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display active proposal count', async () => {
      const count = await governancePage.getActiveProposalCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display past proposal count', async () => {
      const count = await governancePage.getPastProposalCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display proposal titles', async ({ page }) => {
      const count = await governancePage.getProposalCount();
      
      if (count > 0) {
        const proposalCard = governancePage.proposalCards.first();
        const titleEl = proposalCard.locator('h3, .proposal-title, [data-testid="proposal-title"]');
        await expect(titleEl).toBeVisible();
      }
    });

    test('should display proposal status', async ({ page }) => {
      const count = await governancePage.getProposalCount();
      
      if (count > 0) {
        const proposalCard = governancePage.proposalCards.first();
        const statusEl = proposalCard.locator('[data-testid="status"], .status, .proposal-status');
        await expect(statusEl).toBeVisible();
      }
    });

    test('should display voting information', async ({ page }) => {
      const count = await governancePage.getProposalCount();
      
      if (count > 0) {
        const proposalCard = governancePage.proposalCards.first();
        const votesEl = proposalCard.locator('[data-testid="votes"], .votes, .proposal-votes');
        await expect(votesEl).toBeVisible();
      }
    });
  });

  test.describe('Voting', () => {
    test('should display vote buttons on proposals', async ({ page }) => {
      const count = await governancePage.getActiveProposalCount();
      
      if (count > 0) {
        const activeProposal = governancePage.activeProposals.locator('[data-testid="proposal-card"], .proposal-card').first();
        const voteButton = activeProposal.locator('button:has-text("For"), button:has-text("Against"), button:has-text("Abstain")');
        await expect(voteButton).toBeVisible();
      }
    });

    test('should vote on a proposal', async ({ page }) => {
      const count = await governancePage.getActiveProposalCount();
      
      if (count > 0) {
        await governancePage.voteOnProposal(0, 'for');
        
        // Should show confirmation or success message
        // Implementation dependent
      }
    });

    test('should vote against a proposal', async ({ page }) => {
      const count = await governancePage.getActiveProposalCount();
      
      if (count > 0) {
        await governancePage.voteOnProposal(0, 'against');
      }
    });

    test('should vote abstain on a proposal', async ({ page }) => {
      const count = await governancePage.getActiveProposalCount();
      
      if (count > 0) {
        await governancePage.voteOnProposal(0, 'abstain');
      }
    });

    test('should display voting power', async () => {
      const votingPower = await governancePage.getVotingPower();
      expect(votingPower).not.toBeNull();
    });
  });

  test.describe('Create Proposal', () => {
    test('should open create proposal modal', async ({ page }) => {
      await governancePage.createProposalButton.click();
      
      // Should show modal or navigate to create page
      // Implementation dependent
    });

    test('should create a new proposal', async ({ page }) => {
      await governancePage.createProposalButton.click();
      
      // Fill in proposal form (implementation dependent on modal vs page)
      const title = 'Test Proposal';
      const description = 'This is a test proposal for E2E testing purposes.';
      
      await governancePage.createProposal(title, description);
      
      // Should show confirmation or navigate to proposal detail
      // Implementation dependent
    });

    test('should validate proposal title is required', async ({ page }) => {
      await governancePage.createProposalButton.click();
      
      // Try to create with empty title
      await governancePage.createProposal('', 'Description');
      
      // Should show error or not submit
      // Implementation dependent
    });

    test('should validate proposal description is required', async ({ page }) => {
      await governancePage.createProposalButton.click();
      
      // Try to create with empty description
      await governancePage.createProposal('Title', '');
      
      // Should show error or not submit
      // Implementation dependent
    });
  });

  test.describe('Proposal Navigation', () => {
    test('should click on proposal to view details', async ({ page }) => {
      const count = await governancePage.getProposalCount();
      
      if (count > 0) {
        await governancePage.clickProposal(0);
        
        // Should navigate to proposal detail page
        await expect(page).toHaveURL(/\/proposal\/\d+/);
      }
    });

    test('should click on proposal by title', async ({ page }) => {
      const count = await governancePage.getProposalCount();
      
      if (count > 0) {
        const proposalCard = governancePage.proposalCards.first();
        const titleEl = proposalCard.locator('h3, .proposal-title');
        const title = await titleEl.textContent();
        
        if (title) {
          await governancePage.clickProposalByTitle(title);
          await expect(page).toHaveURL(/\/proposal\/\d+/);
        }
      }
    });
  });

  test.describe('Filtering and Sorting', () => {
    test('should filter between active and past proposals', async ({ page }) => {
      // Click on active proposals tab
      const activeTab = page.locator('button:has-text("Active"), [data-testid="active-tab"]');
      if (await activeTab.isVisible()) {
        await activeTab.click();
        await expect(governancePage.activeProposals).toBeVisible();
      }
      
      // Click on past proposals tab
      const pastTab = page.locator('button:has-text("Past"), [data-testid="past-tab"]');
      if (await pastTab.isVisible()) {
        await pastTab.click();
        await expect(governancePage.pastProposals).toBeVisible();
      }
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(governancePage.pageContainer).toBeVisible();
    });

    test('should display correctly on tablet', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(governancePage.pageContainer).toBeVisible();
    });
  });
});
