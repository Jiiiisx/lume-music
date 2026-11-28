import React from 'react';
import { StyleSheet, View } from 'react-native';
import { Tabs } from 'expo-router';
import { BlurView } from 'expo-blur';
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
            position: 'absolute',
            backgroundColor: 'transparent',
            borderTopWidth: 0,
            elevation: 0,
            height: 82,
            paddingTop: 8,
            paddingBottom: 20,
          },
          tabBarBackground: () => (
            <View style={styles.tabBgWrap}>
              <BlurView intensity={75} tint="dark" style={StyleSheet.absoluteFill} />
              <View style={styles.tabBgTint} />
            </View>
          ),
          tabBarActiveTintColor: LumeTheme.crystalWhite,
          tabBarInactiveTintColor: 'rgba(255, 255, 255, 0.48)',
          tabBarLabelStyle: {
            fontSize: 10.5,
            fontWeight: '700',
            letterSpacing: 0.15,
            marginTop: 2,
          },
        }}
      >
        <Tabs.Screen
          name="index"
          options={{
            title: 'Home',
            tabBarIcon: ({ color, size, focused }) => (
              <Ionicons
                name={focused ? 'home' : 'home-outline'}
                size={size - 1}
                color={typeof color === 'string' ? color : LumeTheme.textMuted}
              />
            ),
          }}
        />
        <Tabs.Screen
          name="search"
          options={{
            title: 'Explore',
            tabBarIcon: ({ color, size, focused }) => (
              <Ionicons
                name={focused ? 'compass' : 'compass-outline'}
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
            tabBarIcon: ({ color, size, focused }) => (
              <Ionicons
                name={focused ? 'library' : 'library-outline'}
                size={size - 1}
                color={typeof color === 'string' ? color : LumeTheme.textMuted}
              />
            ),
          }}
        />
        <Tabs.Screen
          name="offline"
          options={{
            title: 'Offline',
            tabBarIcon: ({ color, size, focused }) => (
              <Ionicons
                name={focused ? 'cloud-done' : 'cloud-done-outline'}
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
  tabBgWrap: {
    position: 'absolute',
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    overflow: 'hidden',
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: 'rgba(255, 255, 255, 0.16)',
  },
  tabBgTint: {
    position: 'absolute',
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    backgroundColor: 'rgba(10, 12, 20, 0.62)',
  },
  miniPlayerSlot: {
    position: 'absolute',
    left: 0,
    right: 0,
    bottom: 84,
  },
});
