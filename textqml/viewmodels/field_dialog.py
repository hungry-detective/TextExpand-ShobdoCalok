"""
field_dialog.py — Qt dialog for {field:Label} token fill-in.
Supports multiple fields and displays snippet preview.
Now uses a paragraph-style (FlowLayout) for inline inputs.
"""
import re
from PySide6.QtWidgets import QDialog, QVBoxLayout, QHBoxLayout, QLabel, QLineEdit, QPushButton, QFrame, QScrollArea, QWidget, QTextEdit, QLayout, QSizePolicy, QGridLayout
from PySide6.QtCore import Qt, QPoint, QSize, QRect
from PySide6.QtGui import QFont


class FlowLayout(QLayout):
    """A standard PySide6 FlowLayout implementation to handle wrapping widgets like text."""
    def __init__(self, parent=None, margin=0, spacing=-1):
        super().__init__(parent)
        if parent is not None:
            self.setContentsMargins(margin, margin, margin, margin)
        self.setSpacing(spacing)
        self._items = []

    def __del__(self):
        item = self.takeAt(0)
        while item:
            item = self.takeAt(0)

    def addItem(self, item):
        self._items.append(item)

    def count(self):
        return len(self._items)

    def itemAt(self, index):
        if 0 <= index < len(self._items):
            return self._items[index]
        return None

    def takeAt(self, index):
        if 0 <= index < len(self._items):
            return self._items.pop(index)
        return None

    def expandingDirections(self):
        return Qt.Orientations(0)

    def hasHeightForWidth(self):
        return True

    def heightForWidth(self, width):
        return self._do_layout(QRect(0, 0, width, 0), True)

    def setGeometry(self, rect):
        super().setGeometry(rect)
        self._do_layout(rect, False)

    def sizeHint(self):
        return self.minimumSize()

    def minimumSize(self):
        size = QSize()
        for item in self._items:
            size = size.expandedTo(item.minimumSize())
        m = self.contentsMargins()
        size += QSize(m.left() + m.right(), m.top() + m.bottom())
        return size

    def _do_layout(self, rect, test_only):
        x = rect.x()
        y = rect.y()
        line_height = 0
        spacing = self.spacing()

        for item in self._items:
            wid = item.widget()
            space_x = spacing
            space_y = spacing
            if space_x == -1:
                space_x = wid.style().layoutSpacing(QSizePolicy.PushButton, QSizePolicy.PushButton, Qt.Horizontal)
            if space_y == -1:
                space_y = wid.style().layoutSpacing(QSizePolicy.PushButton, QSizePolicy.PushButton, Qt.Vertical)

            next_x = x + item.sizeHint().width() + space_x
            if next_x - space_x > rect.right() and line_height > 0:
                x = rect.x()
                y = y + line_height + space_y
                next_x = x + item.sizeHint().width() + space_x
                line_height = 0

            if not test_only:
                item.setGeometry(QRect(QPoint(x, y), item.sizeHint()))

            x = next_x
            line_height = max(line_height, item.sizeHint().height())

        return y + line_height - rect.y()


class FieldFillDialog(QDialog):
    def __init__(self, labels: list[str], content_preview: str, is_dark: bool = True, parent=None):
        super().__init__(parent)
        self.setWindowTitle("Shobdo Calok — Fill In Fields")
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Dialog | Qt.WindowType.WindowStaysOnTopHint)
        # Remove setFixedWidth to allow auto-resize
        self.setModal(True)
        self._is_dark = is_dark
        self._drag_pos = None

        self._results = {}
        self._inputs = {}
        self._raw_content = content_preview

        # Theme Colors
        self._bg_color = "#1e293b" if is_dark else "#f8fafc"
        self._scroll_bg = "transparent"
        self._container_bg = "transparent"
        self._text_primary = "#f1f5f9" if is_dark else "#0f172a"
        self._text_secondary = "#94a3b8" if is_dark else "#64748b"
        self._accent = "#a78bfa" if is_dark else "#7c3aed"
        self._border = "#334155" if is_dark else "#e2e8f0"

        self.setStyleSheet(f"QDialog {{ background: {self._bg_color}; border: none; border-radius: 16px; }}")

        # Main layout
        main_layout = QVBoxLayout(self)
        main_layout.setSpacing(12)
        main_layout.setContentsMargins(20, 20, 20, 20)

        # Top Bar (Header + Close Button)
        top_bar = QHBoxLayout()
        top_bar.setContentsMargins(0, 0, 0, 0)
        
        header = QLabel("Complete your form")
        header.setStyleSheet(f"font-size: 15px; color: {"#ffffff" if self._is_dark else "#0f172a"}; font-weight: bold; font-family: 'Inter';")
        top_bar.addStretch()
        top_bar.addWidget(header)
        top_bar.addStretch()
        
        btn_close = QPushButton("close")
        btn_close.setFixedSize(30, 30)
        btn_close.setFont(QFont("Material Symbols Outlined", 16))
        btn_close.setCursor(Qt.CursorShape.PointingHandCursor)
        btn_close.setStyleSheet(f"""
            QPushButton {{
                background: transparent;
                color: {self._text_secondary};
                border: none;
                border-radius: 6px;
                padding-bottom: 2px;
            }}
            QPushButton:hover {{
                background: {"#334155" if is_dark else "#e2e8f0"};
                color: {self._text_primary};
            }}
        """)
        btn_close.clicked.connect(self.reject)
        
        # Absolute position for close button (top right)
        close_container = QWidget()
        close_container.setFixedSize(30, 30)
        close_layout = QVBoxLayout(close_container)
        close_layout.setContentsMargins(0, 0, 0, 0)
        close_layout.addWidget(btn_close)
        
        main_layout.addLayout(top_bar)
        
        # We'll use absolute positioning or a tiny layout for the 'X'
        self._btn_close = btn_close

        # Field Container (Directly in main layout, no ScrollArea)
        self._grid_container = QWidget()
        self._grid_container.setObjectName("formContainer")
        self._grid_container.setStyleSheet("#formContainer { background: transparent; }")
        
        main_layout.addWidget(self._grid_container)

        # Build Flow
        self._parse_and_build_flow(content_preview)

        # Bottom Action Row
        btn_row = QHBoxLayout()
        btn_row.setContentsMargins(0, 5, 0, 0)
        
        btn_ok = QPushButton("Insert Snippet")
        btn_ok.setFixedWidth(180) # Modern centered button
        btn_ok.setDefault(True)
        btn_ok.setCursor(Qt.CursorShape.PointingHandCursor)
        btn_ok.setStyleSheet(f"""
            QPushButton {{
                padding: 10px 24px;
                background: {self._accent};
                color: white;
                border: none;
                border-radius: 10px;
                font-weight: bold;
                font-family: 'Inter';
                font-size: 13px;
            }}
            QPushButton:hover {{
                background: {"#c4b5fd" if is_dark else "#8b5cf6"};
            }}
        """)
        btn_ok.clicked.connect(self._on_accept)
        
        btn_row.addStretch()
        btn_row.addWidget(btn_ok)
        btn_row.addStretch()
        main_layout.addLayout(btn_row)

        # Position close button
        self._btn_close.setParent(self)
        self._btn_close.move(self.width() - 34, 10)

    # ── Draggability ───────────────────────────────────────────────────────────
    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            # Check if we clicked on the dialog itself or a label (not an input)
            child = self.childAt(event.position().toPoint())
            if child is None or isinstance(child, (QLabel, QFrame, QWidget)) and not isinstance(child, QLineEdit):
                self._drag_pos = event.globalPosition().toPoint() - self.frameGeometry().topLeft()
                event.accept()

    def mouseMoveEvent(self, event):
        if event.buttons() == Qt.MouseButton.LeftButton and self._drag_pos is not None:
            self.move(event.globalPosition().toPoint() - self._drag_pos)
            event.accept()

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if hasattr(self, "_btn_close"):
            self._btn_close.move(self.width() - 34, 10)
        event.accept()

    def mouseReleaseEvent(self, event):
        self._drag_pos = None
        event.accept()

    def _parse_and_build_flow(self, text):
        """Build the UI using a Grid Layout for perfect alignment of labels and inputs."""
        lines = text.split("\n")
        token_regex = re.compile(r"(\{field:[^}]+\})")
        
        # Main Grid for all fields
        grid = QGridLayout(self._grid_container)
        grid.setSpacing(10)
        grid.setContentsMargins(0, 0, 0, 0)
        
        # Ensure columns are tight
        grid.setColumnStretch(0, 0)
        grid.setColumnStretch(1, 0)
        
        row_idx = 0
        input_border = "#475569" if self._is_dark else "#cbd5e1"
        input_bg = "#1e293b" if self._is_dark else "#ffffff"
        input_text = "#ffffff" if self._is_dark else "#0f172a"
        
        self._input_widgets = [] # Store in order for non-sync results

        for line in lines:
            # We don't strip the line here to preserve internal spacing if it's meant for context
            parts = token_regex.split(line)
            has_field = False
            
            # Find any field tokens in this line
            for i, part in enumerate(parts):
                if part.startswith("{field:") and part.endswith("}"):
                    has_field = True
                    field_label = part[7:-1]
                    display_text = parts[i-1].strip() if i > 0 else field_label
                    
                    lbl = QLabel(display_text)
                    lbl.setStyleSheet(f"color: {input_text}; font-size: 13px; font-family: 'Inter'; font-weight: 500;")
                    lbl.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
                    
                    edit = QLineEdit()
                    edit.setPlaceholderText("Name")
                    edit.setFixedWidth(180)
                    edit.setFixedHeight(28) # Smart height matching text
                    edit.setStyleSheet(f"""
                        QLineEdit {{
                            padding: 4px 10px;
                            border: 1px solid {input_border};
                            border-radius: 8px;
                            background: {input_bg};
                            color: {input_text};
                            font-family: 'Inter';
                            font-size: 13px;
                        }}
                        QLineEdit:focus {{
                            border: 1px solid {self._accent};
                            background: {"#0f172a" if self._is_dark else "#f8fafc"};
                        }}
                    """)
                    
                    self._input_widgets.append(edit)
                    
                    grid.addWidget(lbl, row_idx, 0)
                    grid.addWidget(edit, row_idx, 1)
                    row_idx += 1

            if not has_field:
                # Add a spacer row for context lines or empty lines to maintain vertical structure
                spacer = QWidget()
                spacer.setFixedHeight(2 if not line.strip() else 18) # Tiny for empty, larger for context text
                if line.strip():
                    context_lbl = QLabel(line.strip())
                    context_lbl.setStyleSheet(f"color: {self._text_secondary}; font-size: 11px; font-style: italic;")
                    grid.addWidget(context_lbl, row_idx, 0, 1, 2)
                    row_idx += 1
                else:
                    grid.addWidget(spacer, row_idx, 0, 1, 2)
                    row_idx += 1
            
        # Call adjustSize to fit content
        self.adjustSize()
        
        # Focus first QLineEdit
        if self._input_widgets:
            self._input_widgets[0].setFocus()

    def _sync_fields(self, label, text, source_edit):
        """No longer used — fields are independent."""
        pass

    def _on_accept(self):
        # Return a LIST of values in order of appearance
        self._results_list = [edit.text() for edit in self._input_widgets]
        self.accept()

    @property
    def results(self) -> list:
        return self._results_list
