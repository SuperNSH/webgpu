// The part of the npm package async-mutex the engine uses (`acquire()`
// resolving to a release function, `isLocked()`): NSH's bundler resolves no
// npm packages.

export class Mutex {
  #locked = false;
  #waiting: (() => void)[] = [];

  isLocked(): boolean {
    return this.#locked;
  }

  async acquire(): Promise<() => void> {
    while (this.#locked) {
      await new Promise<void>((resolve) => this.#waiting.push(resolve));
    }
    this.#locked = true;
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.#locked = false;
      this.#waiting.shift()?.();
    };
  }
}
