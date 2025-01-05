/// <reference types="jest" />

declare global {
    const describe: jest.Describe;
    const it: jest.It;
    const expect: jest.Expect;
    const beforeEach: jest.Hook;
    const afterEach: jest.Hook;
} 