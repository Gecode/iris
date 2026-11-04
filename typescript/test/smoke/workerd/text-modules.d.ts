// Wrangler imports these files as text modules (see wrangler.jsonc).
declare module "*.iris" {
  const text: string;
  export default text;
}

declare module "*.txt" {
  const text: string;
  export default text;
}
