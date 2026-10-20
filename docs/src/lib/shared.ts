import { defineSite } from '@weebo/operator-docs/site';

// What makes this site weebo-skillsagent's; the theme, layout and routes come
// from @weebo/operator-docs (package.json).
export const site = defineSite({
  name: 'weebo-skillsagent',
  description:
    'The Weebo skills, agents and plugins, and wska, the CLI that links them into a project or user Claude Code configuration.',
  git: {
    user: 'batleforc',
    repo: 'weebo-skillsagent',
    branch: 'main',
    contentDir: 'docs/content/docs',
  },
});
