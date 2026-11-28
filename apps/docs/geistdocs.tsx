import type {
  GeistdocsAIConfig,
  GeistdocsNavbarBrand,
} from "@vercel/geistdocs/config";

export const navbarBrand: GeistdocsNavbarBrand = "labs";

export const Logo = () => (
  <span className="-translate-y-[0.5px] font-sans font-semibold text-[24.4px] leading-none tracking-[-0.05em] text-black dark:text-white">
    agent-desktop
  </span>
);

export const github = {
  branch: "main",
  editPath: "apps/docs/content/docs/{path}",
  owner: "vercel-labs",
  repo: "agent-desktop",
};

export const nav = [
  {
    label: "Docs",
    href: "/desktop",
  },
  {
    label: "npm",
    href: "https://www.npmjs.com/package/agent-desktop",
    external: true,
  },
];

export const suggestions = [
  "How do I install agent-desktop?",
  "What platforms does agent-desktop support?",
];

export const title = "agent-desktop";

export const prompt =
  "Answer questions about agent-desktop using the documentation.";

export const ai = {
} satisfies GeistdocsAIConfig;

export const translations = {
  en: {
    displayName: "English",
  },
};

export const basePath: string | undefined = undefined;

/**
 * Unique identifier for this site, used in markdown request tracking analytics.
 * Each site using geistdocs should set this to a unique value (e.g. "ai-sdk-docs", "next-docs").
 */
export const siteId: string | undefined = undefined;
