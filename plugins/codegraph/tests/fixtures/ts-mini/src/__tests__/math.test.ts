import { add, Counter } from '../math';

test('add two positive numbers', () => {
    const result = add(2, 3);
    expect(result).toBe(5);
});

test('counter increments', () => {
    const counter = new Counter();
    counter.increment();
    expect(counter.getValue()).toBe(1);
});
