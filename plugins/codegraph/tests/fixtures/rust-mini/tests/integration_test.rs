use rust_mini::add;

#[test]
fn test_add_positive() {
    let result = add(2, 3);
    assert_eq!(result, 5);
}

#[test]
fn test_add_zero() {
    let result = add(0, 0);
    assert_eq!(result, 0);
}
