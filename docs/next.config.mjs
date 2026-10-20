import { withOperatorDocs } from '@weebo/operator-docs/next';

// Fully static export (see withOperatorDocs). Served from a sub-path on GitHub
// Pages (`/weebo-skillsagent`); set by the docs workflow. Empty locally, so
// `task docs:dev` stays at the root.
export default withOperatorDocs({
  basePath: process.env.DOCS_BASE_PATH ?? '',
});
