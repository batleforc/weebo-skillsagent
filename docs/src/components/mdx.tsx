import { docsMdxComponents } from '@weebo/operator-docs/site';
import type { MDXComponents } from 'mdx/types';

// No CRD here: no guided choices for the manifest builder.
export function getMDXComponents(components?: MDXComponents) {
  return {
    ...docsMdxComponents(),
    ...components,
  } satisfies MDXComponents;
}

export const useMDXComponents = getMDXComponents;

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>;
}
