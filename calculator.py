def add(a, b):
    return a + b

def subtract(a, b):
    return a - b

def multiply(a, b):
    return a * b

def divide(a, b):
    if b == 0:
        raise ZeroDivisionError("Division by zero is not allowed.")
    return a / b

def main():
    while True:
        try:
            print("Enter operation (add, subtract, multiply, divide) or 'quit' to exit:")
            op = input().strip()
            if op == 'quit':
                break
            a = float(input("Enter first number: "))
            b = float(input("Enter second number: "))
            if op == 'add':
                result = add(a, b)
            elif op == 'subtract':
                result = subtract(a, b)
            elif op == 'multiply':
                result = multiply(a, b)
            elif op == 'divide':
                result = divide(a, b)
            else:
                print("Invalid operation.")
                continue
            print(f"Result: {result}")
        except ZeroDivisionError as e:
            print(e)
        except ValueError:
            print("Invalid number entered.")
        except Exception as e:
            print(f"Error: {e}")

if __name__ == "__main__":
    main()