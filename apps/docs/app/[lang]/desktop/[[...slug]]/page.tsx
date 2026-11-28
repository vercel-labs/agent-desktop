import { MobileDocsBar } from "@vercel/geistdocs/mobile-docs-bar";
import { createDocsPage } from "@vercel/geistdocs/pages/docs";
import { getMDXComponents } from "@/components/geistdocs/mdx-components";
import { config } from "@/lib/geistdocs/config";
import { geistdocsSource } from "@/lib/geistdocs/source";
import { versionedOgImage } from "@/lib/og-version";

const docsPage = createDocsPage({
  config,
  mdx: ({ link }) => getMDXComponents({ a: link }),
  source: geistdocsSource,
  openGraph: { images: true },
  metadata: ({ metadata, page }) => {
    const isHome = page.slugs.length === 0;
    const title = isHome ? "agent-desktop | Desktop Automation for AI" : `${page.data.title} | agent-desktop`;
    const description = page.data.description || "Desktop automation CLI for AI agents";
    const url = `https://agent-browser.dev${page.url}`;
    // Match the title the OG route renders so the version tracks card content.
    const imageTitle = isHome ? "Desktop\nAutomation for AI" : (page.data.title ?? "agent-desktop");
    const image = versionedOgImage(geistdocsSource.getPageImage(page).url, imageTitle);
    return {
      ...metadata,
      title: isHome ? { absolute: title } : page.data.title,
      description,
      openGraph: {
        type: "website", locale: "en_US", url, siteName: "agent-desktop", title, description,
        images: [{ url: image, width: 1200, height: 630, alt: `${page.data.title} - agent-desktop` }],
      },
      twitter: {
        card: "summary_large_image", title, description,
        images: [{ url: image, width: 1200, height: 630, alt: `${page.data.title} - agent-desktop` }],
      },
    };
  },
  tableOfContentPopover: {
    enabled: false,
  },
  renderTop: ({ data }) => <MobileDocsBar toc={data.toc} />,
});

export default docsPage.Page;
export const generateStaticParams = docsPage.generateStaticParams;
export const generateMetadata = docsPage.generateMetadata;
