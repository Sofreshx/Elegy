import { add } from './math';

/** Format a sum calculation as a string */
export function formatSum(a: number, b: number): string {
    const result = add(a, b);
    return `${a} + ${b} = ${result}`;
}
