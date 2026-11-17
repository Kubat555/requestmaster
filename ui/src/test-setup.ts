// Runs before every test file (see vitest.config.ts). `svelteTesting` already unmounts rendered components
// after each test. What jsdom lacks is added here.

// jsdom lays nothing out and has no `scrollIntoView`; the tab bar calls it for the shown tab.
Element.prototype.scrollIntoView ??= () => {};
