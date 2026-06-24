//! Start 节点参数/变量项的状态与渲染。
//!
//! 三种类型形态：
//! - **基础类型**（String/Integer/Float/Boolean/DateTime）：name + type + is_optional + is_array + value
//! - **复杂类型**（DataModel 等）：预定义结构，结构只读，值按模式可编辑
//! - **动态类型**（Dynamic）：结构可手动编辑（增删改字段），值按模式可编辑
//!
//! 低代码变量模型规则：
//! - `is_optional=true` 时默认值可省略（UI 提示可选）
//! - `is_array=true` 表示数组/集合类型
//! - 默认值输入控件根据类型变化：Boolean → Switch，其他 → Input
//!
//! 结构类型使用 gpui-component Tree 控件渲染子字段列表，提供：
//! - 统一的缩进与层级展示
//! - 一致的 hover/selection 视觉反馈
//! - 键盘导航支持
//!
//! 子字段值编辑规则：
//! - 参数模式（is_variable=false）：子字段值只读
//! - 变量模式（is_variable=true）：子字段值可编辑

use gpui::{
    div, px, AnyElement, App, AppContext, ClickEvent, Entity, IntoElement, ParentElement, Styled,
    Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::list::ListItem;
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::switch::Switch;
use gpui_component::tree::{TreeItem, TreeState, tree};
use gpui_component::{IconName, Sizable};

use crate::data_type::DataTypeRegistry;
use crate::i18n::{data_type_label, t, Language, TKey};
use crate::theme::Theme;

use super::data_types::{
    item_fields, item_is_array, item_is_optional, item_name, item_type, item_value, value_to_string,
};
use super::StartPanelView;

/// Tree 条目高度（px），用于计算 Tree 容器高度。
const TREE_ENTRY_HEIGHT: f32 = 34.0;

/// 单个子字段的状态。
pub struct FieldState {
    /// 字段名输入（动态类型可编辑，复杂类型只读渲染）。
    pub name: Entity<InputState>,
    /// 字段类型名。
    pub type_value: String,
    /// 字段值输入（变量模式可编辑，参数模式只读渲染）。
    pub value: Entity<InputState>,
}

/// 单个参数/变量项的编辑状态（低代码变量模型）。
pub struct ItemState {
    /// 名称输入。
    pub name: Entity<InputState>,
    /// 当前类型（下拉值）。
    pub type_value: String,
    /// 是否可选（可选时默认值可省略）。
    pub is_optional: bool,
    /// 是否数组/集合。
    pub is_array: bool,
    /// 基础类型的值输入（复杂/动态类型时为 None）。
    pub value: Option<Entity<InputState>>,
    /// 复杂/动态类型的子字段状态。
    pub fields: Vec<FieldState>,
    /// 是否展开显示子字段。
    pub expanded: bool,
    /// 结构类型的 Tree 控件状态（复杂/动态类型时为 Some）。
    pub tree_state: Option<Entity<TreeState>>,
}

impl ItemState {
    /// 从 JSON 值构建项状态。
    ///
    /// `registry` 提供类型元信息（分类、字段定义）。
    pub fn from_value(
        item: &serde_json::Value,
        _is_variable: bool,
        registry: &DataTypeRegistry,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let name_text = item_name(item);
        let type_name = item_type(item);
        let is_optional = item_is_optional(item);
        let is_array = item_is_array(item);
        let has_fields = registry.has_fields(&type_name);

        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(name_text.as_str())
                .placeholder("name")
        });

        let (value, fields) = if has_fields {
            let json_fields = item_fields(item);
            let field_states = build_field_states(&json_fields, window, cx);
            (None, field_states)
        } else {
            let v = item_value(item);
            let val = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(v.as_str())
                    .placeholder("value")
            });
            (Some(val), Vec::new())
        };

        // 为结构类型创建 TreeState
        let tree_state = if has_fields {
            let is_dyn = registry.is_dynamic(&type_name);
            let items = Self::build_tree_items(&fields, is_dyn);
            Some(cx.new(|cx| TreeState::new(cx).items(items)))
        } else {
            None
        };

        Self {
            name,
            type_value: type_name,
            is_optional,
            is_array,
            value,
            fields,
            expanded: has_fields,
            tree_state,
        }
    }

    /// 将项状态序列化为 JSON 值。
    pub fn to_value(&self, cx: &App) -> serde_json::Value {
        let name = self.name.read(cx).value().to_string();
        let type_name = self.type_value.clone();

        if !self.fields.is_empty() || self.value.is_none() {
            let fields: Vec<serde_json::Value> = self
                .fields
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "name": f.name.read(cx).value().to_string(),
                        "type": f.type_value,
                        "value": f.value.read(cx).value().to_string(),
                    })
                })
                .collect();
            serde_json::json!({
                "name": name,
                "type": type_name,
                "is_optional": self.is_optional,
                "is_array": self.is_array,
                "fields": fields,
            })
        } else {
            let val = self
                .value
                .as_ref()
                .map(|e| e.read(cx).value().to_string())
                .unwrap_or_default();
            serde_json::json!({
                "name": name,
                "type": type_name,
                "is_optional": self.is_optional,
                "is_array": self.is_array,
                "value": val,
            })
        }
    }

    /// 构建子字段的 TreeItem 列表。
    ///
    /// TreeItem id 编码字段索引，用于 render 闭包中定位字段状态。
    /// 动态类型末尾追加 "addfield" 条目用于添加字段按钮。
    fn build_tree_items(fields: &[FieldState], is_dynamic: bool) -> Vec<TreeItem> {
        let mut items: Vec<TreeItem> = fields
            .iter()
            .enumerate()
            .map(|(fi, _)| TreeItem::new(format!("field-{fi}"), format!("field-{fi}")))
            .collect();
        if is_dynamic {
            items.push(TreeItem::new("addfield", "+ Add Field"));
        }
        items
    }

    /// 重建 Tree 控件数据（字段增删或类型切换时调用）。
    pub fn rebuild_tree(&mut self, is_dynamic: bool, cx: &mut App) {
        if let Some(tree_state) = &self.tree_state {
            let items = Self::build_tree_items(&self.fields, is_dynamic);
            tree_state.update(cx, |state, cx| {
                state.set_items(items, cx);
            });
        }
    }

    /// 同步项状态从 JSON 值（避免回环）。
    ///
    /// 当类型变化时重建子结构（需要 Window 创建新 InputState）。
    pub fn sync_from_value(
        &mut self,
        item: &serde_json::Value,
        _is_variable: bool,
        registry: &DataTypeRegistry,
        window: &mut Window,
        cx: &mut App,
    ) {
        let name_text = item_name(item);
        let current_name = self.name.read(cx).value().to_string();
        if current_name != name_text {
            self.name.update(cx, |s, cx| {
                s.set_value(name_text.as_str(), window, cx);
            });
        }

        // 同步 is_optional / is_array
        let new_optional = item_is_optional(item);
        let new_array = item_is_array(item);
        self.is_optional = new_optional;
        self.is_array = new_array;

        let type_name = item_type(item);
        let type_changed = type_name != self.type_value;
        let has_fields = registry.has_fields(&type_name);

        if type_changed {
            self.type_value = type_name.clone();
            let was_structured = self.value.is_none();

            if was_structured && !has_fields {
                // 结构 → 基础
                self.fields.clear();
                self.value = Some(cx.new(|cx| {
                    InputState::new(window, cx).placeholder("value")
                }));
                self.expanded = false;
                self.tree_state = None;
            } else if !was_structured && has_fields {
                // 基础 → 结构
                self.value = None;
                self.expanded = true;
                self.rebuild_fields(item, window, cx);
                let is_dyn = registry.is_dynamic(&type_name);
                let tree_items = Self::build_tree_items(&self.fields, is_dyn);
                self.tree_state = Some(cx.new(|cx| TreeState::new(cx).items(tree_items)));
            } else if was_structured && has_fields {
                // 结构 → 结构（类型切换）：重建字段
                self.rebuild_fields(item, window, cx);
                let is_dyn = registry.is_dynamic(&type_name);
                self.rebuild_tree(is_dyn, cx);
            }
        } else if has_fields {
            // 类型未变但字段可能变化（动态类型增删字段）
            self.sync_fields(item, registry, window, cx);
        }

        // 同步值
        if !has_fields {
            let v = item_value(item);
            if let Some(val_entity) = &self.value {
                let current = val_entity.read(cx).value().to_string();
                if current != v {
                    val_entity.update(cx, |s, cx| {
                        s.set_value(v.as_str(), window, cx);
                    });
                }
            }
        }
    }

    /// 重建所有字段（类型切换时）。
    fn rebuild_fields(&mut self, item: &serde_json::Value, window: &mut Window, cx: &mut App) {
        self.fields.clear();
        let json_fields = item_fields(item);
        self.fields = build_field_states(&json_fields, window, cx);
    }

    /// 同步字段值（类型未变，字段数量可能变化）。
    fn sync_fields(
        &mut self,
        item: &serde_json::Value,
        registry: &DataTypeRegistry,
        window: &mut Window,
        cx: &mut App,
    ) {
        let json_fields = item_fields(item);

        if json_fields.len() != self.fields.len() {
            // 字段数量变化：重建
            self.fields = build_field_states(&json_fields, window, cx);
        } else {
            // 逐字段同步
            for (i, f) in json_fields.iter().enumerate() {
                let fname = f
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let ftype = f
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("String")
                    .to_string();
                let fval = f.get("value").map(value_to_string).unwrap_or_default();

                let current_name = self.fields[i].name.read(cx).value().to_string();
                if current_name != fname {
                    self.fields[i].name.update(cx, |s, cx| {
                        s.set_value(fname.as_str(), window, cx);
                    });
                }
                self.fields[i].type_value = ftype;
                let current_val = self.fields[i].value.read(cx).value().to_string();
                if current_val != fval {
                    self.fields[i].value.update(cx, |s, cx| {
                        s.set_value(fval.as_str(), window, cx);
                    });
                }
            }
        }

        // 重建 Tree 数据
        let is_dyn = registry.is_dynamic(&self.type_value);
        self.rebuild_tree(is_dyn, cx);
    }
}

/// 从 JSON 字段数组构建 FieldState 列表。
fn build_field_states(
    json_fields: &[serde_json::Value],
    window: &mut Window,
    cx: &mut App,
) -> Vec<FieldState> {
    json_fields
        .iter()
        .map(|f| {
            let fname = f
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let ftype = f
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("String")
                .to_string();
            let fval = f.get("value").map(value_to_string).unwrap_or_default();
            let name_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(fname.as_str())
                    .placeholder("name")
            });
            let val_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(fval.as_str())
                    .placeholder("value")
            });
            FieldState {
                name: name_input,
                type_value: ftype,
                value: val_input,
            }
        })
        .collect()
}

/// 渲染单个参数/变量项。
///
/// `field_key` 为 "params" 或 "variables"，用于回调标识。
/// `item_idx` 为项在列表中的索引。
/// `is_variable` 区分参数（false）和变量（true），影响子字段值是否可编辑。
/// `registry` 提供类型元信息。
pub fn render_item(
    state: &ItemState,
    field_key: &str,
    item_idx: usize,
    is_variable: bool,
    registry: &DataTypeRegistry,
    lang: Language,
    theme: &Theme,
    entity: &Entity<StartPanelView>,
    cx: &App,
) -> AnyElement {
    let has_fields = registry.has_fields(&state.type_value);
    let is_dynamic = registry.is_dynamic(&state.type_value);
    let is_boolean = state.type_value == "Boolean";

    let mut col = div().flex().flex_col().gap(px(4.0));

    // 第一行：序号 + 名称 + 类型下拉 + 展开/收起(结构类型) + 删除
    col = col.child(render_item_header(
        state,
        field_key,
        item_idx,
        has_fields,
        registry,
        lang,
        theme,
        entity,
        cx,
    ));

    // 第二行：可选/数组开关 + 默认值输入（基础类型）
    col = col.child(render_item_options(
        state,
        field_key,
        item_idx,
        has_fields,
        is_boolean,
        lang,
        theme,
        entity,
        cx,
    ));

    // 结构类型且展开：使用 Tree 控件渲染子字段
    if has_fields && state.expanded {
        if let Some(tree_state) = &state.tree_state {
            col = col.child(render_field_tree(
                tree_state,
                field_key,
                item_idx,
                is_variable,
                is_dynamic,
                registry,
                lang,
                *theme,
                entity,
                state.fields.len(),
            ));
        }
    }

    col.into_any_element()
}

/// 渲染项头部行：序号 + 名称输入 + 类型下拉 + 展开/收起 + 删除。
#[allow(clippy::too_many_arguments)]
fn render_item_header(
    state: &ItemState,
    field_key: &str,
    item_idx: usize,
    has_fields: bool,
    registry: &DataTypeRegistry,
    lang: Language,
    theme: &Theme,
    entity: &Entity<StartPanelView>,
    cx: &App,
) -> AnyElement {
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .p(px(6.0))
        .rounded_md()
        .bg(theme.node_title_bg)
        .border_1()
        .border_color(theme.panel_border);

    // 序号
    row = row.child(
        div()
            .w(px(18.0))
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .text_color(theme.panel_subtext)
            .child(format!("{}", item_idx + 1)),
    );

    // 名称输入
    row = row.child(div().flex_1().child(Input::new(&state.name).appearance(true)));

    // 类型下拉按钮
    let type_btn_id = format!("type-{}-{}", field_key, item_idx);
    let current_type = state.type_value.clone();
    let current_type_label = data_type_label(lang, &current_type).to_string();
    let type_names: Vec<String> = registry
        .type_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let entity_clone = entity.clone();
    let fk = field_key.to_string();

    row = row.child(
        Button::new(type_btn_id)
            .label(current_type_label)
            .icon(IconName::ChevronDown)
            .xsmall()
            .secondary()
            .w(px(100.0))
            .dropdown_menu(move |menu, _w, _cx| {
                let mut menu = menu;
                for ty in &type_names {
                    let ty_val = ty.to_string();
                    let is_checked = ty_val == current_type;
                    let entity = entity_clone.clone();
                    let fk = fk.clone();
                    menu = menu.item(
                        PopupMenuItem::new(ty_val.clone())
                            .checked(is_checked)
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.change_item_type(&fk, item_idx, ty_val.clone(), cx);
                                });
                            }),
                    );
                }
                menu
            }),
    );

    // 展开/收起按钮（仅结构类型）
    if has_fields {
        let expand_btn_id = format!("expand-{}-{}", field_key, item_idx);
        let icon = if state.expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        let entity_clone2 = entity.clone();
        let fk2 = field_key.to_string();
        row = row.child(
            Button::new(expand_btn_id)
                .icon(icon)
                .xsmall()
                .ghost()
                .on_click(move |_: &ClickEvent, _, cx| {
                    entity_clone2.update(cx, |this, cx| {
                        this.toggle_item_expanded(&fk2, item_idx, cx);
                    });
                }),
        );
    }

    // 删除按钮
    let del_btn_id = format!("del-{}-{}", field_key, item_idx);
    let entity_clone3 = entity.clone();
    let fk3 = field_key.to_string();
    row = row.child(
        Button::new(del_btn_id)
            .icon(IconName::Close)
            .xsmall()
            .ghost()
            .on_click(move |_: &ClickEvent, _, cx| {
                entity_clone3.update(cx, |this, cx| {
                    this.delete_item(&fk3, item_idx, cx);
                });
            }),
    );

    let _ = lang;
    let _ = cx;
    row.into_any_element()
}

/// 渲染项选项行：可选开关 + 数组开关 + 默认值输入（基础类型）。
///
/// 低代码规则：
/// - 可选开关：is_optional，开启时默认值可省略
/// - 数组开关：is_array
/// - 默认值输入：基础类型显示，Boolean 用 Switch，其他用 Input
/// - 结构类型不显示默认值（子字段各自有值）
#[allow(clippy::too_many_arguments)]
fn render_item_options(
    state: &ItemState,
    field_key: &str,
    item_idx: usize,
    has_fields: bool,
    is_boolean: bool,
    lang: Language,
    theme: &Theme,
    entity: &Entity<StartPanelView>,
    _cx: &App,
) -> AnyElement {
    let optional_label = t(lang, TKey::PanelParamOptional);
    let array_label = t(lang, TKey::PanelParamArray);
    let value_label = t(lang, TKey::PanelParamValue);

    let mut row = div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .pl(px(30.0))
        .pr(px(6.0));

    // 可选开关
    let opt_id = format!("opt-{}-{}", field_key, item_idx);
    let entity_opt = entity.clone();
    let fk_opt = field_key.to_string();
    row = row.child(
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(
                Switch::new(opt_id)
                    .checked(state.is_optional)
                    .small()
                    .on_click(move |checked: &bool, _, cx| {
                        entity_opt.update(cx, |this, cx| {
                            this.toggle_item_optional(&fk_opt, item_idx, *checked, cx);
                        });
                    }),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme.panel_subtext)
                    .child(optional_label.to_string()),
            ),
    );

    // 数组开关
    let arr_id = format!("arr-{}-{}", field_key, item_idx);
    let entity_arr = entity.clone();
    let fk_arr = field_key.to_string();
    row = row.child(
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(
                Switch::new(arr_id)
                    .checked(state.is_array)
                    .small()
                    .on_click(move |checked: &bool, _, cx| {
                        entity_arr.update(cx, |this, cx| {
                            this.toggle_item_array(&fk_arr, item_idx, *checked, cx);
                        });
                    }),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme.panel_subtext)
                    .child(array_label.to_string()),
            ),
    );

    // 默认值输入（仅基础类型）
    if !has_fields {
        if let Some(val_entity) = &state.value {
            // 占位符：可选时提示可省略
            let placeholder = if state.is_optional {
                format!("{} ({})", value_label, optional_label)
            } else {
                value_label.to_string()
            };

            if is_boolean {
                // Boolean 类型用 Switch 作为默认值控件
                let bool_val = val_entity.read(_cx).value().to_string();
                let checked = bool_val == "true";
                let bool_id = format!("bval-{}-{}", field_key, item_idx);
                let entity_bool = entity.clone();
                let fk_bool = field_key.to_string();
                row = row.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(4.0))
                        .child(
                            Switch::new(bool_id)
                                .checked(checked)
                                .small()
                                .on_click(move |new_checked: &bool, _, cx| {
                                    entity_bool.update(cx, |this, cx| {
                                        this.set_item_value(
                                            &fk_bool,
                                            item_idx,
                                            new_checked.to_string(),
                                            cx,
                                        );
                                    });
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme.panel_subtext)
                                .child(placeholder),
                        ),
                );
            } else {
                // 其他基础类型用 Input
                row = row.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme.panel_subtext)
                                .child(placeholder),
                        )
                        .child(Input::new(val_entity).appearance(true)),
                );
            }
        }
    } else {
        // 结构类型：占位填充保持对齐
        row = row.child(div().flex_1());
    }

    row.into_any_element()
}

/// 使用 Tree 控件渲染结构类型的子字段列表。
#[allow(clippy::too_many_arguments)]
fn render_field_tree(
    tree_state: &Entity<TreeState>,
    field_key: &str,
    item_idx: usize,
    is_variable: bool,
    is_dynamic: bool,
    registry: &DataTypeRegistry,
    lang: Language,
    theme: Theme,
    entity: &Entity<StartPanelView>,
    field_count: usize,
) -> impl IntoElement {
    let entity_clone = entity.clone();
    let fk = field_key.to_string();

    // 预计算类型列表（避免 registry 引用逃逸到 'static 闭包）
    let type_names: Vec<String> = registry
        .type_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let basic_types: Vec<String> = type_names
        .iter()
        .filter(|t| registry.is_basic(t))
        .cloned()
        .collect();

    // 计算 Tree 容器高度
    let entry_count = field_count + if is_dynamic { 1 } else { 0 };
    let tree_height = px(TREE_ENTRY_HEIGHT * entry_count as f32);

    tree(
        tree_state,
        move |ix, entry, _selected, _window, cx| {
            let entity = entity_clone.clone();
            let fk = fk.clone();
            let basic_types = basic_types.clone();
            let theme = theme;

            entity.update(cx, |this, cx| {
                let id = entry.item().id.to_string();

                if id == "addfield" {
                    // 添加字段按钮行
                    render_add_field_entry(ix, &fk, item_idx, &theme, cx)
                } else if let Some(field_idx) = id
                    .strip_prefix("field-")
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    // 字段行
                    let states = if fk == "variables" {
                        &this.variables_state
                    } else {
                        &this.params_state
                    };
                    match states.get(item_idx) {
                        Some(item_state) if field_idx < item_state.fields.len() => {
                            let field_state = &item_state.fields[field_idx];
                            render_field_entry(
                                ix,
                                field_idx,
                                field_state,
                                &fk,
                                item_idx,
                                is_variable,
                                is_dynamic,
                                &basic_types,
                                &theme,
                                lang,
                                cx,
                            )
                        }
                        _ => ListItem::new(ix).child(div()),
                    }
                } else {
                    ListItem::new(ix).child(div())
                }
            })
        },
    )
    .h(tree_height)
    .pl(px(24.0))
}

/// 渲染单个字段行（ListItem）。
#[allow(clippy::too_many_arguments)]
fn render_field_entry(
    ix: usize,
    field_idx: usize,
    field_state: &FieldState,
    field_key: &str,
    item_idx: usize,
    is_variable: bool,
    is_dynamic: bool,
    basic_types: &[String],
    theme: &Theme,
    lang: Language,
    cx: &mut gpui::Context<StartPanelView>,
) -> ListItem {
    let name_entity = field_state.name.clone();
    let val_entity = field_state.value.clone();
    let current_ftype = field_state.type_value.clone();

    let mut row = div().flex().items_center().gap(px(4.0)).w_full().px(px(4.0));

    if is_dynamic {
        // 动态类型：字段名可编辑
        row = row.child(
            div()
                .w(px(64.0))
                .flex_shrink_0()
                .child(Input::new(&name_entity).appearance(true)),
        );

        // 字段类型下拉（仅基础类型可选）
        let field_type_btn_id = format!("ftype-{}-{}-{}", field_key, item_idx, field_idx);
        let entity_clone = cx.entity();
        let fk = field_key.to_string();
        let current_ftype_clone = current_ftype.clone();
        let current_ftype_label = data_type_label(lang, &current_ftype).to_string();
        let basic_types_clone: Vec<String> = basic_types.to_vec();

        row = row.child(
            Button::new(field_type_btn_id)
                .label(current_ftype_label)
                .icon(IconName::ChevronDown)
                .xsmall()
                .ghost()
                .w(px(80.0))
                .dropdown_menu(move |menu, _w, _cx| {
                    let mut menu = menu;
                    for ty in &basic_types_clone {
                        let ty_val = ty.to_string();
                        let is_checked = ty_val == current_ftype_clone;
                        let entity = entity_clone.clone();
                        let fk = fk.clone();
                        menu = menu.item(
                            PopupMenuItem::new(ty_val.clone())
                                .checked(is_checked)
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.change_field_type(
                                            &fk,
                                            item_idx,
                                            field_idx,
                                            ty_val.clone(),
                                            cx,
                                        );
                                    });
                                }),
                        );
                    }
                    menu
                }),
        );

        // 删除字段按钮
        let del_field_btn_id = format!("fdel-{}-{}-{}", field_key, item_idx, field_idx);
        let entity_clone2 = cx.entity();
        let fk2 = field_key.to_string();
        row = row.child(
            Button::new(del_field_btn_id)
                .icon(IconName::Close)
                .xsmall()
                .ghost()
                .on_click(move |_: &ClickEvent, _, cx| {
                    entity_clone2.update(cx, |this, cx| {
                        this.delete_field(&fk2, item_idx, field_idx, cx);
                    });
                }),
        );

        // 字段值
        if is_variable {
            row = row.child(
                div()
                    .flex_1()
                    .child(Input::new(&val_entity).appearance(true)),
            );
        } else {
            let val = field_state.value.read(cx).value().to_string();
            row = row.child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(theme.panel_subtext)
                    .child(val),
            );
        }
    } else {
        // 复杂类型：字段名只读
        let fname = field_state.name.read(cx).value().to_string();
        row = row.child(
            div()
                .w(px(64.0))
                .flex_shrink_0()
                .text_size(px(12.0))
                .text_color(theme.panel_label_text)
                .child(fname),
        );

        // 字段类型只读 tag
        row = row.child(
            div()
                .w(px(64.0))
                .flex_shrink_0()
                .px(px(4.0))
                .py(px(2.0))
                .rounded_sm()
                .bg(theme.toolbar_toggle_bg)
                .text_size(px(11.0))
                .text_color(theme.toolbar_toggle_text)
                .flex()
                .items_center()
                .justify_center()
                .child(field_state.type_value.clone()),
        );

        // 字段值
        if is_variable {
            row = row.child(
                div()
                    .flex_1()
                    .child(Input::new(&val_entity).appearance(true)),
            );
        } else {
            let val = field_state.value.read(cx).value().to_string();
            row = row.child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(theme.panel_subtext)
                    .child(val),
            );
        }
    }

    let _ = lang;
    ListItem::new(ix).w_full().child(row)
}

/// 渲染"添加字段"按钮行（ListItem）。
fn render_add_field_entry(
    ix: usize,
    field_key: &str,
    item_idx: usize,
    _theme: &Theme,
    cx: &mut gpui::Context<StartPanelView>,
) -> ListItem {
    let add_btn_id = format!("fadd-{}-{}", field_key, item_idx);
    let entity = cx.entity();
    let fk = field_key.to_string();

    ListItem::new(ix).w_full().child(
        div().w_full().child(
            Button::new(add_btn_id)
                .icon(IconName::Plus)
                .xsmall()
                .ghost()
                .on_click(move |_: &ClickEvent, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.add_field(&fk, item_idx, cx);
                    });
                }),
        ),
    )
}
