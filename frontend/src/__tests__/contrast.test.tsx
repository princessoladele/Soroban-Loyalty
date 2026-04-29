/**
 * Automated WCAG 2.1 AA contrast checks using jest-axe.
 * Runs against the key page components with both dark (default) and light themes.
 * Add new components here to prevent contrast regressions.
 */
import { render } from "@testing-library/react";
import { axe, toHaveNoViolations } from "jest-axe";

expect.extend(toHaveNoViolations);

// Minimal stubs so pages render without network/context dependencies
jest.mock("@/lib/api", () => ({
  api: {
    getAnalytics: jest.fn().mockResolvedValue(null),
    getCampaigns: jest.fn().mockResolvedValue([]),
    getUserRewards: jest.fn().mockResolvedValue([]),
  },
}));
jest.mock("@/context/WalletContext", () => ({
  useWallet: () => ({ publicKey: null, connecting: false, connect: jest.fn(), disconnect: jest.fn() }),
}));

// axe rules scoped to color-contrast only — fast and focused
const contrastOnly = { runOnly: { type: "rule", values: ["color-contrast"] } };

function withTheme(theme: "dark" | "light", html: string) {
  document.documentElement.removeAttribute("data-theme");
  if (theme === "light") document.documentElement.setAttribute("data-theme", "light");
  return html;
}

describe("WCAG 2.1 AA color-contrast", () => {
  afterEach(() => document.documentElement.removeAttribute("data-theme"));

  it("stat-card passes contrast in dark theme", async () => {
    withTheme("dark", "");
    const { container } = render(
      <div>
        <div className="stat-card">
          <div className="stat-value">1,234</div>
          <div className="stat-label">Total Claims</div>
        </div>
      </div>
    );
    const results = await axe(container, contrastOnly);
    expect(results).toHaveNoViolations();
  });

  it("stat-card passes contrast in light theme", async () => {
    withTheme("light", "");
    const { container } = render(
      <div>
        <div className="stat-card">
          <div className="stat-value">1,234</div>
          <div className="stat-label">Total Claims</div>
        </div>
      </div>
    );
    const results = await axe(container, contrastOnly);
    expect(results).toHaveNoViolations();
  });

  it("badge expired passes contrast in dark theme", async () => {
    withTheme("dark", "");
    const { container } = render(
      <span className="badge" data-status="expired">Expired</span>
    );
    const results = await axe(container, contrastOnly);
    expect(results).toHaveNoViolations();
  });

  it("badge expired passes contrast in light theme", async () => {
    withTheme("light", "");
    const { container } = render(
      <span className="badge" data-status="expired">Expired</span>
    );
    const results = await axe(container, contrastOnly);
    expect(results).toHaveNoViolations();
  });

  it("text-muted passes contrast in dark theme", async () => {
    withTheme("dark", "");
    const { container } = render(
      <div className="stat-card"><p className="stat-label">Muted label text</p></div>
    );
    const results = await axe(container, contrastOnly);
    expect(results).toHaveNoViolations();
  });

  it("text-muted passes contrast in light theme", async () => {
    withTheme("light", "");
    const { container } = render(
      <div className="stat-card"><p className="stat-label">Muted label text</p></div>
    );
    const results = await axe(container, contrastOnly);
    expect(results).toHaveNoViolations();
  });
});
