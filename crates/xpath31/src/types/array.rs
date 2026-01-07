use super::{XdmItem, XdmValue};
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct XdmArray<N> {
    members: Vec<XdmValue<N>>,
}

impl<N: Clone> XdmArray<N> {
    pub fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            members: Vec::with_capacity(capacity),
        }
    }

    pub fn from_members(members: Vec<XdmValue<N>>) -> Self {
        Self { members }
    }

    pub fn get(&self, index: usize) -> Option<&XdmValue<N>> {
        if index == 0 {
            return None;
        }
        self.members.get(index - 1)
    }

    pub fn put(&self, index: usize, value: XdmValue<N>) -> Option<Self> {
        if index == 0 || index > self.members.len() {
            return None;
        }
        let mut new_arr = self.clone();
        new_arr.members[index - 1] = value;
        Some(new_arr)
    }

    pub fn append(&self, value: XdmValue<N>) -> Self {
        let mut new_arr = self.clone();
        new_arr.members.push(value);
        new_arr
    }

    pub fn insert_before(&self, index: usize, value: XdmValue<N>) -> Option<Self> {
        if index == 0 || index > self.members.len() + 1 {
            return None;
        }
        let mut new_arr = self.clone();
        new_arr.members.insert(index - 1, value);
        Some(new_arr)
    }

    pub fn remove(&self, index: usize) -> Option<Self> {
        if index == 0 || index > self.members.len() {
            return None;
        }
        let mut new_arr = self.clone();
        new_arr.members.remove(index - 1);
        Some(new_arr)
    }

    pub fn subarray(&self, start: usize, length: usize) -> Option<Self> {
        if start == 0 || start > self.members.len() {
            return None;
        }
        let end = std::cmp::min(start - 1 + length, self.members.len());
        Some(Self::from_members(self.members[start - 1..end].to_vec()))
    }

    pub fn head(&self) -> Option<&XdmValue<N>> {
        self.members.first()
    }

    pub fn tail(&self) -> Option<Self> {
        if self.members.is_empty() {
            return None;
        }
        Some(Self::from_members(self.members[1..].to_vec()))
    }

    pub fn reverse(&self) -> Self {
        let mut reversed = self.members.clone();
        reversed.reverse();
        Self::from_members(reversed)
    }

    pub fn join(arrays: &[XdmArray<N>]) -> Self {
        let total_len: usize = arrays.iter().map(|a| a.members.len()).sum();
        let mut result = Vec::with_capacity(total_len);
        for arr in arrays {
            result.extend(arr.members.iter().cloned());
        }
        Self::from_members(result)
    }

    pub fn size(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn members(&self) -> &[XdmValue<N>] {
        &self.members
    }

    pub fn iter(&self) -> impl Iterator<Item = &XdmValue<N>> {
        self.members.iter()
    }

    pub fn into_items(self) -> Vec<XdmItem<N>> {
        vec![XdmItem::Array(self)]
    }

    pub fn flatten(&self) -> Vec<XdmItem<N>>
    where
        N: Clone,
    {
        let mut result = Vec::new();
        for member in &self.members {
            match member {
                XdmValue::Sequence(items) => {
                    for item in items {
                        if let XdmItem::Array(arr) = item {
                            result.extend(arr.flatten());
                        } else {
                            result.push(item.clone());
                        }
                    }
                }
            }
        }
        result
    }
}

impl<N: Clone> Default for XdmArray<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N: PartialEq + Clone> PartialEq for XdmArray<N> {
    fn eq(&self, other: &Self) -> bool {
        self.members == other.members
    }
}

impl<N: Eq + Clone> Eq for XdmArray<N> {}

impl<N: Hash + Clone> Hash for XdmArray<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.members.len().hash(state);
        for member in &self.members {
            member.hash(state);
        }
    }
}

impl<N: fmt::Debug> fmt::Display for XdmArray<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        let mut first = true;
        for member in &self.members {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", member)?;
            first = false;
        }
        write!(f, "]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AtomicValue;

    fn int_val<N: Clone>(i: i64) -> XdmValue<N> {
        XdmValue::from_atomic(AtomicValue::Integer(i))
    }

    #[test]
    fn test_array_operations() {
        let arr: XdmArray<()> = XdmArray::new();
        assert!(arr.is_empty());
        assert_eq!(arr.size(), 0);

        let arr = arr.append(int_val(1));
        assert_eq!(arr.size(), 1);

        let arr = arr.append(int_val(2)).append(int_val(3));
        assert_eq!(arr.size(), 3);

        assert!(arr.get(1).is_some());
        assert!(arr.get(0).is_none());
        assert!(arr.get(4).is_none());
    }

    #[test]
    fn test_array_subarray() {
        let arr: XdmArray<()> = XdmArray::from_members(vec![
            int_val(1),
            int_val(2),
            int_val(3),
            int_val(4),
            int_val(5),
        ]);

        let sub = arr.subarray(2, 3).unwrap();
        assert_eq!(sub.size(), 3);

        let sub = arr.subarray(4, 10).unwrap();
        assert_eq!(sub.size(), 2);
    }

    #[test]
    fn test_array_head_tail() {
        let arr: XdmArray<()> = XdmArray::from_members(vec![int_val(1), int_val(2), int_val(3)]);

        assert!(arr.head().is_some());
        let tail = arr.tail().unwrap();
        assert_eq!(tail.size(), 2);
    }

    #[test]
    fn test_array_reverse() {
        let arr: XdmArray<()> = XdmArray::from_members(vec![int_val(1), int_val(2), int_val(3)]);
        let reversed = arr.reverse();
        assert_eq!(reversed.size(), 3);
    }

    #[test]
    fn test_array_join() {
        let arr1: XdmArray<()> = XdmArray::from_members(vec![int_val(1), int_val(2)]);
        let arr2: XdmArray<()> = XdmArray::from_members(vec![int_val(3), int_val(4)]);

        let joined = XdmArray::join(&[arr1, arr2]);
        assert_eq!(joined.size(), 4);
    }

    #[test]
    fn test_array_equality_compares_contents() {
        let arr1: XdmArray<()> = XdmArray::from_members(vec![int_val(1), int_val(2)]);
        let arr2: XdmArray<()> = XdmArray::from_members(vec![int_val(1), int_val(2)]);
        let arr3: XdmArray<()> = XdmArray::from_members(vec![int_val(1), int_val(3)]);
        let arr4: XdmArray<()> = XdmArray::from_members(vec![int_val(1)]);

        // Same contents should be equal
        assert_eq!(arr1, arr2);

        // Different contents (same length) should NOT be equal
        assert_ne!(arr1, arr3);

        // Different length should NOT be equal
        assert_ne!(arr1, arr4);
    }

    #[test]
    fn test_array_hash_includes_contents() {
        use std::collections::hash_map::DefaultHasher;

        fn hash_array(arr: &XdmArray<()>) -> u64 {
            let mut hasher = DefaultHasher::new();
            arr.hash(&mut hasher);
            hasher.finish()
        }

        let arr1: XdmArray<()> = XdmArray::from_members(vec![int_val(1), int_val(2)]);
        let arr2: XdmArray<()> = XdmArray::from_members(vec![int_val(1), int_val(2)]);
        let arr3: XdmArray<()> = XdmArray::from_members(vec![int_val(1), int_val(3)]);

        // Same contents should have same hash
        assert_eq!(hash_array(&arr1), hash_array(&arr2));

        // Different contents (same length) should have different hash
        assert_ne!(hash_array(&arr1), hash_array(&arr3));
    }
}
