(() => {
  const create = (tag, props = {}, text) => {
    const el = document.createElement(tag);
    Object.entries(props).forEach(([key, value]) => {
      if (key === 'style' && value && typeof value === 'object') {
        Object.assign(el.style, value);
      } else if (key === 'className') {
        el.className = value;
      } else {
        el.setAttribute(key, value);
      }
    });
    if (text !== undefined) el.textContent = text;
    return el;
  };

  const makeCalculator = () => {
    const state = {
      display: '0',
      storedValue: null,
      operator: null,
      waitingForOperand: false,
      justEvaluated: false,
    };

    const updateDisplay = (value, inputEl) => {
      inputEl.value = value;
    };

    const performCalculation = (first, second, op) => {
      switch (op) {
        case '+': return first + second;
        case '-': return first - second;
        case '*': return first * second;
        case '/':
          if (second === 0) throw new Error('Cannot divide by zero');
          return first / second;
        default: return second;
      }
    };

    const calculate = (inputEl) => {
      const current = Number.parseFloat(state.display);
      if (state.storedValue === null || state.operator === null) {
        state.storedValue = current;
        return;
      }

      const result = performCalculation(state.storedValue, current, state.operator);
      state.display = String(result);
      updateDisplay(state.display, inputEl);
      state.storedValue = result;
      state.justEvaluated = true;
      state.waitingForOperand = true;
    };

    const appendDigit = (digit, inputEl) => {
      if (state.waitingForOperand) {
        state.display = digit;
        state.waitingForOperand = false;
      } else {
        state.display = state.display === '0' ? digit : state.display + digit;
      }
      state.justEvaluated = false;
      updateDisplay(state.display, inputEl);
    };

    const appendDecimal = (inputEl) => {
      if (state.justEvaluated) {
        state.display = '0';
        state.justEvaluated = false;
      }
      if (state.waitingForOperand) {
        state.display = '0.';
        state.waitingForOperand = false;
      } else if (!state.display.includes('.')) {
        state.display += '.';
      }
      updateDisplay(state.display, inputEl);
    };

    const handleOperator = (nextOperator, inputEl) => {
      const inputValue = Number.parseFloat(state.display);

      if (state.storedValue !== null && state.operator !== null && !state.waitingForOperand) {
        const result = performCalculation(state.storedValue, inputValue, state.operator);
        state.display = String(result);
        updateDisplay(state.display, inputEl);
        state.storedValue = result;
      } else {
        state.storedValue = inputValue;
      }

      state.operator = nextOperator;
      state.waitingForOperand = true;
      state.justEvaluated = false;
    };

    const clearAll = (inputEl) => {
      state.display = '0';
      state.storedValue = null;
      state.operator = null;
      state.waitingForOperand = false;
      state.justEvaluated = false;
      updateDisplay(state.display, inputEl);
    };

    const toggleSign = (inputEl) => {
      const value = Number.parseFloat(state.display) * -1;
      state.display = String(value);
      updateDisplay(state.display, inputEl);
    };

    const app = create('div', {
      className: 'calculator',
      style: {
        width: '320px',
        margin: '40px auto',
        padding: '20px',
        borderRadius: '16px',
        background: '#1f2937',
        boxShadow: '0 12px 30px rgba(0,0,0,0.25)',
        fontFamily: 'Arial, sans-serif',
      }
    });

    const display = create('input', {
      type: 'text',
      value: '0',
      readonly: 'true',
      style: {
        width: '100%',
        boxSizing: 'border-box',
        fontSize: '2rem',
        textAlign: 'right',
        border: 'none',
        outline: 'none',
        borderRadius: '10px',
        padding: '16px 14px',
        marginBottom: '16px',
        background: '#111827',
        color: '#f9fafb',
      }
    });

    const buttonGrid = create('div', {
      style: {
        display: 'grid',
        gridTemplateColumns: 'repeat(4, minmax(0, 1fr))',
        gap: '10px',
      }
    });

    const addButton = (label, action, variant = 'normal') => {
      const btn = create('button', {
        type: 'button',
        style: {
          padding: '18px 10px',
          fontSize: '1.2rem',
          border: 'none',
          borderRadius: '10px',
          cursor: 'pointer',
          background: variant === 'operator' ? '#f59e0b' : variant === 'special' ? '#374151' : '#e5e7eb',
          color: variant === 'normal' ? '#111827' : '#fff',
          fontWeight: '700',
        }
      }, label);

      btn.addEventListener('click', () => action(btn, display));
      return btn;
    };

    buttonGrid.append(
      addButton('C', clearAll, 'special'),
      addButton('±', toggleSign, 'special'),
      addButton('%', () => {
        const value = Number.parseFloat(state.display) / 100;
        state.display = String(value);
        updateDisplay(state.display, display);
      }, 'special'),
      addButton('÷', () => handleOperator('/', display), 'operator'),
      addButton('7', () => appendDigit('7', display)),
      addButton('8', () => appendDigit('8', display)),
      addButton('9', () => appendDigit('9', display)),
      addButton('×', () => handleOperator('*', display), 'operator'),
      addButton('4', () => appendDigit('4', display)),
      addButton('5', () => appendDigit('5', display)),
      addButton('6', () => appendDigit('6', display)),
      addButton('-', () => handleOperator('-', display), 'operator'),
      addButton('1', () => appendDigit('1', display)),
      addButton('2', () => appendDigit('2', display)),
      addButton('3', () => appendDigit('3', display)),
      addButton('+', () => handleOperator('+', display), 'operator'),
      addButton('0', () => appendDigit('0', display), 'normal'),
      addButton('.', () => appendDecimal(display), 'normal'),
      addButton('=', () => {
        try {
          calculate(display);
        } catch (error) {
          state.display = 'Error';
          updateDisplay(state.display, display);
          state.storedValue = null;
          state.operator = null;
          state.waitingForOperand = true;
        }
      }, 'operator')
    );

    app.append(display, buttonGrid);

    return app;
  };

  if (typeof document !== 'undefined') {
    const existing = document.getElementById('calculator-root');
    const root = existing || document.createElement('div');
    if (!existing) {
      root.id = 'calculator-root';
      document.body.appendChild(root);
    }
    root.innerHTML = '';
    root.appendChild(makeCalculator());
    return;
  }

  const readline = (() => {
    try {
      return require('readline').createInterface({
        input: process.stdin,
        output: process.stdout,
      });
    } catch (_error) {
      return null;
    }
  })();

  if (readline) {
    const ask = (question) => new Promise((resolve) => readline.question(question, resolve));
    (async () => {
      const first = await ask('Enter first number: ');
      const operator = await ask('Enter operator (+, -, *, /): ');
      const second = await ask('Enter second number: ');
      const a = Number(first);
      const b = Number(second);
      if (Number.isNaN(a) || Number.isNaN(b)) {
        console.log('Invalid number input.');
      } else {
        switch (operator) {
          case '+': console.log(a + b); break;
          case '-': console.log(a - b); break;
          case '*': console.log(a * b); break;
          case '/': console.log(b === 0 ? 'Cannot divide by zero' : a / b); break;
          default: console.log('Invalid operator.');
        }
      }
      readline.close();
    })();
  }
})();
