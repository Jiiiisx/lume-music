import React from 'react';
import { StyleSheet, View } from 'react-native';
import { Tabs } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { MiniPlayer } from '../../src/components/MiniPlayer';
import { LumeTheme } from '../../src/theme';

export default function TabsLayout() {
  return (
    <View style={styles.root}>
      <Tabs
        screenOptions={{
          headerShown: false,
          sceneStyle: { backgroundColor: LumeTheme.bg },
          tabBarStyle: {
            backgroundColor: '#0E121D',
            borderTopColor: LumeTheme.border,
            borderTopWidth: 1,
            height: 78,
            paddingTop: 8,
            paddingBottom: 18,
          },
          tabBarActiveTintColor: LumeTheme.accentBright,
          tabBarInactiveTintColor: LumeTheme.textMuted,
          tabBarLabelStyle: {
            fontSize: 11,
            fontWeight: '600',
          },
        }}
      >
        <Tabs.Screen
          name="index"
          options={{
            title: 'Home',
            tabBarIcon: ({ color, size }) => (
              <Ionicons
                name="home-outline"
                size={size}
                color={typeof color === 'string' ? color : LumeTheme.textMuted}
              />
            ),
          }}
        />
        <Tabs.Screen
          name="search"
          options={{
            title: 'Search',
            tabBarIcon: ({ color, size }) => (
              <Ionicons
                name="search-outline"
                size={size}
                color={typeof color === 'string' ? color : LumeTheme.textMuted}
              />
            ),
          }}
        />
        <Tabs.Screen
          name="library"
          options={{
            title: 'Library',
            tabBarIcon: ({ color, size }) => (
              <Ionicons
                name="library-outline"
                size={size}
                color={typeof color === 'string' ? color : LumeTheme.textMuted}
              />
            ),
          }}
        />
        <Tabs.Screen
          name="offline"
          options={{
            title: 'Offline',
            tabBarIcon: ({ color, size }) => (
              <Ionicons
                name="cloud-done-outline"
                size={size}
                color={typeof color === 'string' ? color : LumeTheme.textMuted}
              />
            ),
          }}
        />
      </Tabs>

      <View style={styles.miniPlayerSlot} pointerEvents="box-none">
        <MiniPlayer />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    backgroundColor: LumeTheme.bg,
  },
  miniPlayerSlot: {
    position: 'absolute',
    left: 0,
    right: 0,
    bottom: 80,
  },
});
