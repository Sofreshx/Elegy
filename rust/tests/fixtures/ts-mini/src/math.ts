/**
 * Basic math utilities.
 */

/** Add two numbers together */
export function add(a: number, b: number): number {
    return a + b;
}

/** Internal helper — not exported */
function multiply(a: number, b: number): number {
    return a * b;
}

/** A simple counter class */
export class Counter {
    private count: number = 0;

    increment(): void {
        this.count++;
    }

    getValue(): number {
        return this.count;
    }
}
