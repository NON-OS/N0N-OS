class AsyncQueue<T> {
    private queue: T[] = [];
    private resolvers: ((value: T) => void)[] = [];

    push(item: T) {
        if (this.resolvers.length > 0) {
            const resolve = this.resolvers.shift()!;
            resolve(item);
        } else {
            this.queue.push(item);
        }
    }

    async pop(): Promise<T> {
        if (this.queue.length > 0) {
            return this.queue.shift()!;
        }

        return new Promise<T>(resolve => this.resolvers.push(resolve));
    }

    isEmpty(): boolean {
        return this.queue.length === 0;
    }

    clear() {
        this.queue = [];
        this.resolvers = [];
    }
}

class AsyncEvent {
    private flag = false;
    private resolvers: (() => void)[] = [];

    set() {
        this.flag = true;
        for (const resolve of this.resolvers) resolve();
        this.resolvers = [];
    }

    clear() {
        this.flag = false;
    }

    async wait(): Promise<void> {
        if (this.flag) return;
        return new Promise(resolve => this.resolvers.push(resolve));
    }
}

export { AsyncQueue, AsyncEvent };
