/** Version de l'application, celle de package.json au moment du build. */
export const versionApplication: string = __VERSION__;

/** Texte affiché dans la fenêtre : « Version 0.1.0 ». */
export function libelleVersion(version: string): string {
  return `Version ${version}`;
}
