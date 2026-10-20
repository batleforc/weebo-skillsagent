import Link from 'next/link';
import { repoUrl } from '@weebo/operator-docs/site';
import { site } from '@/lib/shared';

// The package's OperatorHome is an operator's landing page ("Install the
// chart", an example CR): this repo has neither, so this page lays out the
// same proof sheet with the theme's classes and its own words.

const features = [
  {
    title: 'Guide',
    body: 'wska by use case: a new project, a fresh clone, the user scope, Gemini CLI and Antigravity, authoring or vendoring a skill, team repos, CI, Windows, troubleshooting.',
    href: '/docs/guide',
    link: 'Find your situation',
  },
  {
    title: 'Reference',
    body: 'Every wska command and its exit codes, the ownership rules that decide what it will and will not touch, the profiles.toml and vendor.toml schemas.',
    href: '/docs/wska',
    link: 'CLI and config',
  },
  {
    title: 'judge',
    body: 'Headless review gates a hook or CI job can act on: rfc-ready and doc-review, each printing one JSON report and a meaningful exit code.',
    href: '/docs/judge',
    link: 'Review gates',
  },
];

export default function HomePage() {
  return (
    <main className="flex flex-1 flex-col px-4 sm:px-6">
      <section className="mx-auto flex w-full max-w-[1100px] flex-col gap-6 pt-10 pb-10 sm:pt-16">
        <span className="sheet-label">Weebo skills, agents and plugins</span>
        <h1 className="sheet-display">{site.name}</h1>
        <p className="sheet-caption">
          One repository holding the Claude Code skills used across every project, plus wska — the
          CLI that links a chosen subset into a project or into the user&apos;s global ~/.claude/.
        </p>
        <div className="flex flex-wrap items-center gap-3">
          <Link href="/docs" className="action">
            Read the docs
          </Link>
          <Link href="/docs/guide" className="ctl">
            Guide by use case
          </Link>
          <a href={repoUrl(site)} className="ctl" rel="noreferrer">
            View on GitHub
          </a>
        </div>
      </section>

      <section className="sheet-region mx-auto w-full max-w-[1100px]">
        <div className="grid gap-6 md:grid-cols-2">
          <div className="flex min-w-0 flex-col gap-3">
            <span className="sheet-label">1 · Link a profile</span>
            <pre className="sheet-pre">
              <code>{`wska install --profile weebo --run-installers`}</code>
            </pre>
          </div>
          <div className="flex min-w-0 flex-col gap-3">
            <span className="sheet-label">2 · Check it works</span>
            <pre className="sheet-pre">
              <code>{`wska doctor`}</code>
            </pre>
          </div>
        </div>
      </section>

      <section className="sheet-region mx-auto grid w-full max-w-[1100px] md:grid-cols-3">
        {features.map((f) => (
          <div key={f.title} className="sheet-feature flex flex-col gap-3 py-6 md:px-6 md:py-0 md:first:ps-0">
            <h2>{f.title}</h2>
            <p className="text-[13px] leading-[1.6] text-fd-muted-foreground">{f.body}</p>
            <Link href={f.href}>{f.link}</Link>
          </div>
        ))}
      </section>
    </main>
  );
}
